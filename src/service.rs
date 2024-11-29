use lnd_grpc_rust::lnrpc;
use lnd_grpc_rust::lnrpc::channel_point::FundingTxid;
use log::{debug, info, warn};
use std::env;

use crate::api::orders::OrderStatus;
use crate::mempool;
use crate::{api::Api, node::LNNode};

pub struct Service {
    node: LNNode,
    api: Api,
}

impl Service {
    pub fn new(node: LNNode, api: Api) -> Self {
        Self { node, api }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Checking orders...");

        let orders = self.api.get_orders().await?;

        for order in orders {
            match order.status {
                OrderStatus::WAITING_FOR_CHANNEL_OPEN => {
                    info!("Opening channel for order: {}", order.id);
                    self.open_channel(&order).await?;
                }

                OrderStatus::WAITING_FOR_SELLER_APPROVAL => {
                    info!("Approving order: {}", order.id);
                    self.process_new_order(&order).await?;
                }

                _ => {
                    debug!("Skipping order: {} ({:?})", order.id, order.status);
                }
            }
        }

        Ok(())
    }

    /**
     * 1. Get current fee rate
     * 2. Calculate UTXOs required and fees
     * 3. Ensure it's profitable
     * 4. Create channel
     * 5. Confirm channel open
     *
     * @param order
     * @returns
     */
    async fn open_channel(
        &self,
        order: &crate::api::orders::OrdersGetUserMarketOfferOrdersList,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Opening channel for order: {}", order.id);

        let channel_size: i64 = order.size.parse().unwrap();
        let node_pubkey = order.account.to_owned().into_bytes();

        // 1. Get current fee rate
        let sat_per_vbyte = mempool::get_fastest_fee().await?;
        debug!("Current fee rate: {}", sat_per_vbyte);

        // 2. Calculate UTXOs required and fees
        let utxos = self.node.list_unspent().await?;
        let outpoints = calculate_utxos_required_and_fees(channel_size, sat_per_vbyte, utxos)?;
        debug!("Using {} UTXOs: {:?}", outpoints.len(), outpoints);

        // 3. Ensure it's profitable
        let fee = calc_fee(outpoints.len(), sat_per_vbyte);
        let order_cost = order
            .seller_invoice_amount
            .as_ref()
            .map(|s| s.parse::<i64>().unwrap())
            .unwrap();

        if fee > order_cost as f64 {
            Err(format!(
                "Fee is higher than order cost. Fee: {}, Order cost: {}",
                fee, order_cost
            ))?;
        }
        info!("Expected profit: {} sats", order_cost as f64 - fee);

        // 4. Create channel
        let channel_point = self
            .node
            .open_channel(node_pubkey, sat_per_vbyte as u64, channel_size, outpoints)
            .await?;

        let tx_hex = match &channel_point.funding_txid {
            Some(FundingTxid::FundingTxidBytes(bytes)) => hex::encode(bytes),
            Some(FundingTxid::FundingTxidStr(txid_str)) => txid_str.clone(),
            None => Err("No funding txid")?,
        };

        let tx_point = format!("{}:{}", tx_hex, channel_point.output_index);
        info!("Channel opened: https://mempool.space/tx/{}", tx_point);

        // 5. Confirm channel open
        self.api
            .confirm_channel_open(order.id.as_str(), tx_point.as_str())
            .await
    }

    /**
     * 1. Get buyer's address
     * 2. Make sure we can connect to buyer's node
     *  - If not, reject order
     * 3. Create invoice
     * 4. Accept order
     *
     * @param order
     * @returns
     */
    async fn process_new_order(
        &self,
        order: &crate::api::orders::OrdersGetUserMarketOfferOrdersList,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let reject_if_off = env::var("REJECT_IF_BUYER_OFFLINE")
            .map(|val| val == "true")
            .unwrap_or(true);

        info!("Processing new order: {}", order.id);
        if reject_if_off {
            // 1. Get buyer's address
            let pubkey: String = order.account.clone();
            let addresses = self.api.get_node_addresses(&pubkey).await?;
            let addr = addresses.first().unwrap();

            // 2. Make sure we can connect to buyer's node
            let buyer_info = self.node.check_connect_to_node(addr, &pubkey).await;
            debug!("Successfully connected to buyer's node");

            if let Err(e) = buyer_info {
                warn!("Can't connect to buyer's node, rejecting order. {}", e);
                self.api.reject_order(order.id.as_str()).await?;

                return Ok(());
            }
        } else {
            info!("Skipping buyer's node check");
        }

        // 3. Create invoice
        let order_cost = order
            .seller_invoice_amount
            .as_ref()
            .unwrap()
            .parse::<i64>()
            .unwrap();
        let invoice = self.node.create_invoice(order_cost, 172800).await?;
        debug!("Invoice created: {}", invoice);

        // 4. Accept order
        self.api
            .accept_order(order.id.as_str(), invoice.as_str())
            .await?;

        Ok(())
    }
}

fn calculate_utxos_required_and_fees(
    channel_size: i64,
    sat_per_vbyte: u8,
    utxos: Vec<lnrpc::Utxo>,
) -> Result<Vec<lnrpc::OutPoint>, Box<dyn std::error::Error>> {
    let total: i64 = utxos.iter().map(|utxo| utxo.amount_sat).sum();
    let mut amount_remaining = channel_size as f64;

    let mut related_outpoints = vec![];

    if total < channel_size {
        return Err(format!(
            "There are no UTXOs available to open a channel of {} sats. Total UTXOS: {} sats",
            amount_remaining, total
        ))?;
    }

    for utxo in utxos {
        related_outpoints.push(utxo.outpoint.unwrap().clone());

        let fee_cost = calc_fee(related_outpoints.len(), sat_per_vbyte);
        let amount_with_fees = channel_size as f64 + fee_cost;

        if amount_remaining <= amount_with_fees {
            amount_remaining = 0.0;
            break;
        }
        amount_remaining -= utxo.amount_sat as f64;
    }

    if amount_remaining > 0.0 {
        return Err(format!(
            "There are no UTXOs available to open a channel of {} sats. Total UTXOS: {} sats short",
            channel_size, amount_remaining
        ))?;
    }

    Ok(related_outpoints)
}

fn calc_fee(num_inputs: usize, sat_per_vbyte: u8) -> f64 {
    let transaction_size = tx_size(num_inputs);
    let fee = transaction_size * sat_per_vbyte as f64;
    fee
}

fn tx_size(utxos_needed: usize) -> f64 {
    let inputs_size = utxos_needed as f64 * 57.5;
    let outputs_size = 2.0 * 43.0;
    let overhead_size = 10.5;
    let total_size = inputs_size + outputs_size + overhead_size;
    total_size
}
