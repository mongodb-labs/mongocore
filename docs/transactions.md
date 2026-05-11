# Transactions

MongoCore provides multi-document ACID transactions via gRPC. Transactions are managed server-side using a concurrent session map — your client simply holds a transaction ID.

## How It Works

1. Call `BeginTransaction` — MongoCore starts a session and returns a `transaction_id`
2. Pass that `transaction_id` with every CRUD operation in the transaction
3. Call `CommitTransaction` or `AbortTransaction` when done

MongoCore manages the MongoDB session lifecycle internally using DashMap for lock-free concurrent access.

## Python

```python
from mongocore import MongoClient

async def transfer_funds(client, from_account, to_account, amount):
    accounts = client["bank"]["accounts"]

    # Begin transaction
    tx_id = await client.begin_transaction("bank")

    try:
        # Debit source
        await accounts.update_one(
            {"_id": from_account},
            {"$inc": {"balance": -amount}},
            transaction_id=tx_id
        )

        # Credit destination
        await accounts.update_one(
            {"_id": to_account},
            {"$inc": {"balance": amount}},
            transaction_id=tx_id
        )

        # Commit
        await client.commit_transaction(tx_id)
        print("Transfer successful")

    except Exception as e:
        await client.abort_transaction(tx_id)
        print(f"Transfer failed, rolled back: {e}")
```

## TypeScript

```typescript
import { MongoClient } from '@mongocore/client';

async function transferFunds(
  client: MongoClient,
  fromAccount: string,
  toAccount: string,
  amount: number
) {
  const accounts = client.db('bank').collection('accounts');
  const txId = await client.beginTransaction('bank');

  try {
    await accounts.updateOne(
      { _id: fromAccount },
      { $inc: { balance: -amount } },
      { transactionId: txId }
    );

    await accounts.updateOne(
      { _id: toAccount },
      { $inc: { balance: amount } },
      { transactionId: txId }
    );

    await client.commitTransaction(txId);
    console.log('Transfer successful');
  } catch (e) {
    await client.abortTransaction(txId);
    console.error(`Transfer failed, rolled back: ${e}`);
  }
}
```

## Go

```go
func transferFunds(ctx context.Context, client *mongocore.Client, from, to string, amount int64) error {
    accounts := client.Database("bank").Collection("accounts")

    txID, err := client.BeginTransaction(ctx, "bank")
    if err != nil {
        return err
    }

    opts := &mongocore.TxOptions{TransactionID: txID}

    _, err = accounts.UpdateOne(ctx,
        bson.D{{Key: "_id", Value: from}},
        bson.D{{Key: "$inc", Value: bson.D{{Key: "balance", Value: -amount}}}},
        opts,
    )
    if err != nil {
        client.AbortTransaction(ctx, txID)
        return err
    }

    _, err = accounts.UpdateOne(ctx,
        bson.D{{Key: "_id", Value: to}},
        bson.D{{Key: "$inc", Value: bson.D{{Key: "balance", Value: amount}}}},
        opts,
    )
    if err != nil {
        client.AbortTransaction(ctx, txID)
        return err
    }

    return client.CommitTransaction(ctx, txID)
}
```

## Java

```java
public void transferFunds(MongoClient client, String from, String to, long amount)
        throws Exception {
    MongoCollection accounts = client.getDatabase("bank").getCollection("accounts");

    String txId = client.beginTransaction("bank");

    try {
        accounts.updateOne(
            new Document("_id", from),
            new Document("$inc", new Document("balance", -amount)),
            txId
        );

        accounts.updateOne(
            new Document("_id", to),
            new Document("$inc", new Document("balance", amount)),
            txId
        );

        client.commitTransaction(txId);
    } catch (Exception e) {
        client.abortTransaction(txId);
        throw e;
    }
}
```

## gRPC Protocol

```protobuf
message BeginTransactionRequest {
  string database = 1;
}

message BeginTransactionResponse {
  string transaction_id = 1;
}

message CommitTransactionRequest {
  string transaction_id = 1;
}

message AbortTransactionRequest {
  string transaction_id = 1;
}
```

## Notes

- Transactions require a MongoDB replica set or sharded cluster (not standalone)
- MongoCore uses DashMap for concurrent session management — multiple transactions can run simultaneously without lock contention
- Transaction sessions are automatically cleaned up if the client disconnects
- The `transaction_id` is an opaque string — don't parse or construct it yourself
