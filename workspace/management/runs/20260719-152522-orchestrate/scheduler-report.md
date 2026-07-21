# Scheduler Report

- Enforced max parallel: `2`
- Planned batches: `6`

## Batch 1

- Issues: #11, #15
- Width: `2`
- Status: `completed`
- Message: existing committed worker reports passed verification

## Batch 2

- Issues: #13
- Width: `1`
- Status: `blocked`
- Message: scheduler batch 2 failed worker verification

## Batch 3

- Issues: #14
- Width: `1`
- Status: `blocked`
- Message: not dispatched because scheduler batch 2 failed worker verification

## Batch 4

- Issues: #12
- Width: `1`
- Status: `blocked`
- Message: not dispatched because scheduler batch 2 failed worker verification

## Batch 5

- Issues: #16
- Width: `1`
- Status: `blocked`
- Message: not dispatched because scheduler batch 2 failed worker verification

## Batch 6

- Issues: #17
- Width: `1`
- Status: `blocked`
- Message: not dispatched because scheduler batch 2 failed worker verification
