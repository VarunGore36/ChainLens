# API Usage Examples

## Quick Reference

### Get block by number

```bash
curl http://127.0.0.1:8080/block/18000000
```

### Get transaction by hash

```bash
curl http://127.0.0.1:8080/transaction/0xabc123...
```

### Explain a transaction

```bash
curl http://127.0.0.1:8080/api/v1/transactions/0xabc123.../explain
```

Response:
```json
{
  "tx_hash": "0xabc123...",
  "block_number": 18000000,
  "from": "0x1111...",
  "to": "0x2222...",
  "value_eth": "1.000000",
  "status": "success",
  "actions": [
    {
      "action_type": "eth_transfer",
      "description": "Transferred 1.000000 ETH"
    }
  ],
  "summary": "Transferred 1.000000 ETH from 0x1111... to 0x2222..."
}
```

### Get address intelligence

```bash
curl http://127.0.0.1:8080/api/v1/addresses/0x1111.../intelligence
```

Response:
```json
{
  "address": "0x1111...",
  "total_transactions": 150,
  "total_eth_sent": "25.500000",
  "total_eth_received": "30.200000",
  "unique_contracts_called": 12,
  "behavior_classifications": ["active_user", "token_trader"],
  "activity_level": "high"
}
```

### Get address relationship graph

```bash
curl "http://127.0.0.1:8080/api/v1/addresses/0x1111.../graph?depth=2&limit=50"
```

### Get contract intelligence

```bash
curl http://127.0.0.1:8080/api/v1/contracts/0xUniswapRouter.../intelligence
```

### Get anomalies

```bash
curl http://127.0.0.1:8080/api/v1/anomalies
```

### Get block analytics with MEV detection

```bash
curl http://127.0.0.1:8080/api/v1/blocks/18000000/analytics
```

### Export address transactions as JSON

```bash
curl "http://127.0.0.1:8080/api/v1/addresses/0x1111.../export?format=json&limit=1000"
```

### Export address transactions as CSV

```bash
curl "http://127.0.0.1:8080/api/v1/addresses/0x1111.../export?format=csv&limit=1000" -o transactions.csv
```

### Get address activity trends

```bash
curl "http://127.0.0.1:8080/api/v1/addresses/0x1111.../trends?days=30"
```

### Get contract interaction trends

```bash
curl "http://127.0.0.1:8080/api/v1/contracts/0xUniswapRouter.../trends?days=30"
```

### Get anomaly trends

```bash
curl "http://127.0.0.1:8080/api/v1/anomalies/trends?days=30"
```

### Get MEV trends

```bash
curl "http://127.0.0.1:8080/api/v1/mev/trends?days=30"
```

### Find address cluster

```bash
curl "http://127.0.0.1:8080/api/v1/addresses/0x1111.../cluster?depth=3"
```

### Get contract deployers

```bash
curl http://127.0.0.1:8080/api/v1/contracts/deployers
```

### Get token whales

```bash
curl http://127.0.0.1:8080/api/v1/tokens/0xUSDC.../whales
```

### Health check

```bash
curl http://127.0.0.1:8080/health
```

Response:
```json
{
  "status": "ok",
  "database": "connected",
  "version": "0.1.0"
}
```

### Indexer status

```bash
curl http://127.0.0.1:8080/status
```

### API documentation

```bash
curl http://127.0.0.1:8080/docs
```

### WebSocket connection

```javascript
const ws = new WebSocket('ws://127.0.0.1:8080/ws');

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    console.log(data);
};
```

Events:
- `block_committed` — new block indexed
- `anomaly_detected` — anomaly found
- `mev_detected` — MEV pattern found
- `reorg_detected` — chain reorganization

## Python Examples

### Get address intelligence

```python
import requests

response = requests.get('http://127.0.0.1:8080/api/v1/addresses/0x1111.../intelligence')
data = response.json()
print(f"Transactions: {data['total_transactions']}")
print(f"ETH Sent: {data['total_eth_sent']}")
print(f"Classifications: {data['behavior_classifications']}")
```

### Export transactions to CSV

```python
import requests
import csv
import io

response = requests.get('http://127.0.0.1:8080/api/v1/addresses/0x1111.../export?format=csv&limit=1000')
reader = csv.DictReader(io.StringIO(response.text))

for row in reader:
    print(f"{row['block_number']}: {row['from']} -> {row['to']} ({row['value']} wei)")
```

### Monitor WebSocket

```python
import websocket
import json

def on_message(ws, message):
    data = json.loads(message)
    print(f"Event: {data['type']}")
    if data['type'] == 'block_committed':
        print(f"  Block #{data['block_number']}: {data['tx_count']} transactions")
    elif data['type'] == 'anomaly_detected':
        print(f"  {data['severity']}: {data['description']}")

ws = websocket.WebSocketApp('ws://127.0.0.1:8080/ws', on_message=on_message)
ws.run_forever()
```

## JavaScript Examples

### Fetch and display anomalies

```javascript
const response = await fetch('http://127.0.0.1:8080/api/v1/anomalies');
const anomalies = await response.json();

anomalies.forEach(a => {
    console.log(`[${a.severity}] ${a.description}`);
});
```

### Real-time block monitoring

```javascript
const ws = new WebSocket('ws://127.0.0.1:8080/ws');

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    
    switch(data.type) {
        case 'block_committed':
            updateBlockDisplay(data.block_number, data.tx_count);
            break;
        case 'anomaly_detected':
            showNotification(data.description, data.severity);
            break;
    }
};
```
