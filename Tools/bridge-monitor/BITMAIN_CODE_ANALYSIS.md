# Bitmain ASIC Code Analysis & Verification

**Status**: Code-Verified (Hardware Not Available for Testing)  
**Purpose**: Document all Bitmain-specific logic to enable safe refactoring

---

## ⚠️ CRITICAL: Untested Code Paths

This document analyzes Bitmain-specific code paths that **cannot be tested with real hardware**. All refactoring must preserve this logic exactly through code verification.

---

## Bitmain Detection Logic

### Code Location
- `client_handler.rs::assign_extranonce_for_miner()` (lines 91-93)
- `default_client.rs::handle_subscribe()` (lines 110-112)
- `default_client.rs::send_extranonce()` (lines 355-357)
- `client_handler.rs::send_immediate_job_to_client()` (lines 312-314)
- `client_handler.rs::new_block_available()` (lines 604-606)

### Detection Code (PRESERVE EXACTLY)
```rust
let remote_app_lower = remote_app.to_lowercase();
let is_bitmain = remote_app_lower.contains("godminer") || 
                remote_app_lower.contains("bitmain") ||
                remote_app_lower.contains("antminer");
```

### Verification Checklist
- [x] Case-insensitive matching (`.to_lowercase()`)
- [x] Three keyword checks: "godminer", "bitmain", "antminer"
- [x] Uses `contains()` for substring matching
- [x] Logic: OR condition (matches if any keyword found)

**Refactoring Rule**: This detection logic appears in 5+ places. Extract to function but preserve logic exactly.

---

## Extranonce Handling

### Bitmain Extranonce Size: 0

**Code Location**: `client_handler.rs::assign_extranonce_for_miner()` (lines 84-129)

```rust
let required_extranonce_size = if is_bitmain { 0 } else { 2 };

let extranonce = if required_extranonce_size > 0 {
    // ... assignment logic for non-Bitmain
} else {
    // Bitmain path
    tracing::debug!("[AUTO-EXTRANONCE] Assigned empty extranonce (size: 0 bytes) to Bitmain miner '{}'", remote_app);
    String::new()  // Empty string for Bitmain
};
```

**Key Points**:
- Bitmain gets empty extranonce string (`String::new()`)
- Extranonce counter is NOT incremented for Bitmain
- This allows unlimited Bitmain connections (no extranonce pool limit)

**Verification**:
- [x] Bitmain path returns empty string
- [x] Extranonce counter not touched for Bitmain
- [x] No warning messages for Bitmain

---

### Subscribe Response Format

**Code Location**: `default_client.rs::handle_subscribe()` (lines 121-133)

**Bitmain Format**:
```rust
if is_bitmain {
    let extranonce2_size = 8 - (extranonce.len() / 2);
    JsonRpcResponse::new(
        &event,
        Some(Value::Array(vec![
            Value::Null,                    // First element: null
            Value::String(extranonce.clone()), // Second: extranonce (will be empty for Bitmain)
            Value::Number(extranonce2_size.into()), // Third: extranonce2_size
        ])),
        None,
    )
}
```

**Other ASICs Format**:
```rust
else {
    JsonRpcResponse::new(
        &event,
        Some(Value::Array(vec![
            Value::Bool(true),
            Value::String("EthereumStratum/1.0.0".to_string()),
        ])),
        None,
    )
}
```

**Key Differences**:
1. Bitmain: First element is `null` (others: `true`)
2. Bitmain: Includes extranonce string (even if empty)
3. Bitmain: Includes `extranonce2_size` number
4. Bitmain: Three elements total (others: two elements)

**extranonce2_size Calculation**:
```rust
let extranonce2_size = 8 - (extranonce.len() / 2);
```
- For Bitmain: extranonce is empty (length 0), so extranonce2_size = 8 - 0 = 8
- This means Bitmain uses full 8-byte extranonce2 field

**Verification**:
- [x] Array has 3 elements for Bitmain
- [x] First element is `Value::Null`
- [x] Second element is extranonce (empty string for Bitmain)
- [x] Third element is extranonce2_size (8 for Bitmain)
- [x] Calculation: 8 - (extranonce.len() / 2)

---

### Extranonce Message Format

**Code Location**: `default_client.rs::send_extranonce()` (lines 360-373)

**Bitmain Format**:
```rust
if is_bitmain {
    let extranonce2_size = 8 - (extranonce.len() / 2);
    vec![
        Value::String(extranonce.clone()),
        Value::Number(extranonce2_size.into()),
    ]
}
```

**Other ASICs Format**:
```rust
else {
    vec![Value::String(extranonce.clone())]
}
```

**Key Differences**:
- Bitmain: Two parameters `[extranonce, extranonce2_size]`
- Others: One parameter `[extranonce]`

**Notification Format**:
- IceRiver: Minimal format (no id/jsonrpc)
- Bitmain: Standard JSON-RPC format (with id/jsonrpc)

**Verification**:
- [x] Bitmain extranonce message has 2 parameters
- [x] First parameter: extranonce string (empty for Bitmain)
- [x] Second parameter: extranonce2_size number (8 for Bitmain)
- [x] Bitmain uses standard JSON-RPC format (not minimal)

---

## Job Format

### Bitmain Uses Legacy Array Format

**Code Location**: 
- `client_handler.rs::send_immediate_job_to_client()` (lines 344-353)
- `client_handler.rs::new_block_available()` (lines 628-636)

**Bitmain Job Format**:
```rust
if is_iceriver {
    // IceRiver: single hex string
} else if state.use_big_job() && !is_iceriver {
    // BzMiner: single hex string (big-endian)
} else {
    // Bitmain/Legacy: array + number format
    let header_bytes = pre_pow_hash.as_bytes();
    let job_header = generate_job_header(&header_bytes);
    job_params.push(serde_json::Value::Array(
        job_header.iter().map(|&v| serde_json::Value::Number(v.into())).collect()
    ));
    job_params.push(serde_json::Value::Number(block.header.timestamp.into()));
}
```

**Format Details**:
- `job_params[0]`: Job ID (string)
- `job_params[1]`: Array of 4 u64 values (from `generate_job_header()`)
- `job_params[2]`: Timestamp (number)

**generate_job_header() Function**:
- Location: `hasher.rs::generate_job_header()` (lines 263-280)
- Input: Header bytes (32 bytes)
- Output: `Vec<u64>` with 4 elements
- Conversion: Little-endian u64 values from header bytes

**Verification**:
- [x] Bitmain uses `generate_job_header()` function
- [x] NOT using `generate_iceriver_job_params()`
- [x] NOT using `generate_large_job_params()`
- [x] Format: Array + Number (not single hex string)
- [x] Array contains exactly 4 u64 values
- [x] Timestamp added as separate Number parameter

---

## Notification Format

### Bitmain Uses Standard JSON-RPC

**Code Location**: 
- `client_handler.rs::send_immediate_job_to_client()` (lines 380-392)
- `client_handler.rs::new_block_available()` (lines 657-669)

**IceRiver Format** (Minimal):
```rust
if is_iceriver {
    client_clone.send_notification("mining.notify", job_params.clone()).await
}
```

**Bitmain Format** (Standard JSON-RPC):
```rust
else {
    let notify_event = JsonRpcEvent {
        jsonrpc: "2.0".to_string(),
        method: "mining.notify".to_string(),
        id: Some(serde_json::Value::Number(job_id.into())),
        params: job_params.clone(),
    };
    client_clone.send(notify_event).await
}
```

**Key Differences**:
- Bitmain: Includes `jsonrpc: "2.0"` field
- Bitmain: Includes `id` field (job_id)
- Bitmain: Uses `send()` method (full JSON-RPC)
- IceRiver: Uses `send_notification()` method (minimal format)

**Verification**:
- [x] Bitmain notification includes `jsonrpc` field
- [x] Bitmain notification includes `id` field
- [x] Bitmain uses standard JSON-RPC format
- [x] NOT using minimal format (IceRiver-only)

---

## Code Locations Summary

### Files with Bitmain-Specific Code

1. **client_handler.rs**
   - Line 91-93: Bitmain detection
   - Line 95: Extranonce size assignment (0 for Bitmain)
   - Line 119-121: Empty extranonce assignment
   - Line 312-314: Bitmain detection (job formatting)
   - Line 344-353: Legacy array format (Bitmain path)
   - Line 604-606: Bitmain detection (new_block_available)
   - Line 628-636: Legacy array format (Bitmain path)

2. **default_client.rs**
   - Line 110-112: Bitmain detection (subscribe)
   - Line 121-133: Bitmain subscribe response format
   - Line 355-357: Bitmain detection (extranonce)
   - Line 360-373: Bitmain extranonce message format

3. **hasher.rs**
   - Lines 263-280: `generate_job_header()` function (used by Bitmain)

---

## Refactoring Safety Rules

### ✅ SAFE Refactoring (Preserves Logic)

1. **Extract Detection Function**
   ```rust
   pub fn is_bitmain_miner(remote_app: &str) -> bool {
       // EXACT copy of existing logic
       let remote_app_lower = remote_app.to_lowercase();
       remote_app_lower.contains("godminer") || 
       remote_app_lower.contains("bitmain") ||
       remote_app_lower.contains("antminer")
   }
   ```
   - ✅ Preserves exact logic
   - ✅ Improves maintainability
   - ✅ Makes testing easier

2. **Extract Format Generation**
   ```rust
   fn format_bitmain_job_params(pre_pow_hash: &Hash, timestamp: u64, job_id: u64) -> Vec<Value> {
       // EXACT copy of existing logic
       let mut params = vec![Value::String(job_id.to_string())];
       let header_bytes = pre_pow_hash.as_bytes();
       let job_header = generate_job_header(&header_bytes);
       params.push(Value::Array(
           job_header.iter().map(|&v| Value::Number(v.into())).collect()
       ));
       params.push(Value::Number(timestamp.into()));
       params
   }
   ```
   - ✅ Preserves exact format
   - ✅ Can be unit tested
   - ✅ Reduces duplication

### ❌ UNSAFE Refactoring (Changes Logic)

1. **Changing Detection Strings**
   - ❌ Don't change "godminer", "bitmain", "antminer"
   - ❌ Don't change case-insensitive matching
   - ❌ Don't change OR logic to AND logic

2. **Changing Format Functions**
   - ❌ Don't modify `generate_job_header()` output format
   - ❌ Don't change array size (must be 4 u64 values)
   - ❌ Don't change endianness

3. **Changing Protocol Formats**
   - ❌ Don't change subscribe response format
   - ❌ Don't change extranonce message format
   - ❌ Don't change notification format (must be standard JSON-RPC)

---

## Unit Test Strategy

Since hardware testing is not available, create comprehensive unit tests:

```rust
#[cfg(test)]
mod bitmain_tests {
    use super::*;
    
    #[test]
    fn test_bitmain_detection() {
        assert!(is_bitmain_miner("GodMiner v1.0"));
        assert!(is_bitmain_miner("bitmain-asic"));
        assert!(is_bitmain_miner("ANTMINER-KS"));
        assert!(!is_bitmain_miner("IceRiver KS2L"));
        assert!(!is_bitmain_miner("BzMiner"));
    }
    
    #[test]
    fn test_bitmain_extranonce_size() {
        let size = get_extranonce_size_for_miner("GodMiner");
        assert_eq!(size, 0);
    }
    
    #[test]
    fn test_bitmain_subscribe_response() {
        let response = create_subscribe_response_bitmain("", 8);
        // Verify: [null, "", 8] format
        assert!(matches!(response.result, Some(Value::Array(ref arr)) if arr.len() == 3));
    }
    
    #[test]
    fn test_bitmain_job_format() {
        let hash = /* test hash */;
        let params = format_bitmain_job_params(&hash, 1234567890, 1);
        // Verify: [job_id, [u64, u64, u64, u64], timestamp]
        assert_eq!(params.len(), 3);
        assert!(params[1].is_array());
        if let Value::Array(arr) = &params[1] {
            assert_eq!(arr.len(), 4); // 4 u64 values
        }
        assert!(params[2].is_number());
    }
}
```

---

## Verification Workflow

Before any refactoring that touches Bitmain code:

1. **Document Current Behavior**
   - [ ] List all Bitmain-specific code paths
   - [ ] Document expected outputs
   - [ ] Create unit tests

2. **Refactor**
   - [ ] Extract code to functions
   - [ ] Preserve logic exactly
   - [ ] Add documentation comments

3. **Verify**
   - [ ] Run unit tests (should pass)
   - [ ] Review git diff (no logic changes)
   - [ ] Compare byte-level output (format generation)
   - [ ] Code review focusing on Bitmain paths

4. **Document**
   - [ ] Update this analysis document
   - [ ] Add comments in code: `// BITMAIN-SPECIFIC: Code-verified, hardware not available`
   - [ ] Note what was changed and why

---

**Last Updated**: 2024  
**Status**: Code Analysis for Untested Bitmain Code Paths  
**Testing**: Unit tests only - hardware not available
