# Amboss Magma Bot

This bot will monitor the Amboss Magma API and:

- React to new orders:
  - Check if buyer's node is reachable
  - Create an invoice
  - Confirm the order
- React to new payments:
  - Check current network fee
  - Open a channel if profitable
  - Confirm the channel opening to Amboss
