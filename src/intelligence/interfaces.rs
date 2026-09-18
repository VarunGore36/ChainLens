use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug)]
pub struct InterfaceInfo {
    pub name: String,
    pub description: String,
}

static KNOWN_INTERFACES: LazyLock<HashMap<[u8; 4], InterfaceInfo>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // ERC-20
    m.insert(
        [0xa9, 0x05, 0x9c, 0xbb],
        InterfaceInfo {
            name: "ERC-20".to_string(),
            description: "transfer(address,uint256)".to_string(),
        },
    );
    m.insert(
        [0x09, 0x5e, 0xa7, 0xb3],
        InterfaceInfo {
            name: "ERC-20".to_string(),
            description: "approve(address,uint256)".to_string(),
        },
    );
    m.insert(
        [0x23, 0xb8, 0x72, 0xdd],
        InterfaceInfo {
            name: "ERC-20/721".to_string(),
            description: "transferFrom(address,address,uint256)".to_string(),
        },
    );
    m.insert(
        [0xdd, 0x62, 0xed, 0x3e],
        InterfaceInfo {
            name: "ERC-20".to_string(),
            description: "allowance(address,address)".to_string(),
        },
    );
    m.insert(
        [0x70, 0xa0, 0x82, 0x31],
        InterfaceInfo {
            name: "ERC-20".to_string(),
            description: "balanceOf(address)".to_string(),
        },
    );
    m.insert(
        [0x18, 0x16, 0x0d, 0xdd],
        InterfaceInfo {
            name: "ERC-20".to_string(),
            description: "totalSupply()".to_string(),
        },
    );

    // ERC-721
    m.insert(
        [0x42, 0x84, 0x2e, 0x0e],
        InterfaceInfo {
            name: "ERC-721".to_string(),
            description: "safeTransferFrom(address,address,uint256)".to_string(),
        },
    );
    m.insert(
        [0xb8, 0x8d, 0x4f, 0xde],
        InterfaceInfo {
            name: "ERC-721".to_string(),
            description: "safeTransferFrom(address,address,uint256,bytes)".to_string(),
        },
    );
    m.insert(
        [0x08, 0x18, 0x12, 0xfc],
        InterfaceInfo {
            name: "ERC-721".to_string(),
            description: "getApproved(uint256)".to_string(),
        },
    );
    m.insert(
        [0xa2, 0x2c, 0xb4, 0x65],
        InterfaceInfo {
            name: "ERC-721".to_string(),
            description: "setApprovalForAll(address,bool)".to_string(),
        },
    );
    m.insert(
        [0xe9, 0x85, 0xe9, 0xc5],
        InterfaceInfo {
            name: "ERC-721".to_string(),
            description: "isApprovedForAll(address,address)".to_string(),
        },
    );
    m.insert(
        [0x63, 0x52, 0x21, 0x1e],
        InterfaceInfo {
            name: "ERC-721".to_string(),
            description: "ownerOf(uint256)".to_string(),
        },
    );

    // ERC-1155
    m.insert(
        [0xf2, 0x42, 0x43, 0x2a],
        InterfaceInfo {
            name: "ERC-1155".to_string(),
            description: "safeTransferFrom(address,address,uint256,uint256,bytes)".to_string(),
        },
    );
    m.insert(
        [0x2e, 0xb2, 0xc2, 0xd6],
        InterfaceInfo {
            name: "ERC-1155".to_string(),
            description: "safeBatchTransferFrom(address,address,uint256[],uint256[],bytes)"
                .to_string(),
        },
    );
    m.insert(
        [0x00, 0xfd, 0xd5, 0x8e],
        InterfaceInfo {
            name: "ERC-1155".to_string(),
            description: "balanceOf(address,uint256)".to_string(),
        },
    );
    m.insert(
        [0x4e, 0x12, 0x63, 0xb9],
        InterfaceInfo {
            name: "ERC-1155".to_string(),
            description: "balanceOfBatch(address[],uint256[])".to_string(),
        },
    );
    m.insert(
        [0xa2, 0x2c, 0xb4, 0x65],
        InterfaceInfo {
            name: "ERC-1155".to_string(),
            description: "setApprovalForAll(address,bool)".to_string(),
        },
    );

    // Uniswap V2 Router
    m.insert(
        [0x38, 0xed, 0x17, 0x39],
        InterfaceInfo {
            name: "Uniswap V2".to_string(),
            description: "swapExactTokensForTokens".to_string(),
        },
    );
    m.insert(
        [0x88, 0x03, 0xdb, 0xee],
        InterfaceInfo {
            name: "Uniswap V2".to_string(),
            description: "swapTokensForExactTokens".to_string(),
        },
    );
    m.insert(
        [0x7f, 0xf3, 0x6a, 0xb5],
        InterfaceInfo {
            name: "Uniswap V2".to_string(),
            description: "swapExactETHForTokens".to_string(),
        },
    );
    m.insert(
        [0x4a, 0x25, 0xd9, 0x4a],
        InterfaceInfo {
            name: "Uniswap V2".to_string(),
            description: "swapTokensForExactETH".to_string(),
        },
    );
    m.insert(
        [0x18, 0xcb, 0xaf, 0xe5],
        InterfaceInfo {
            name: "Uniswap V2".to_string(),
            description: "swapExactTokensForETH".to_string(),
        },
    );
    m.insert(
        [0xe8, 0xe3, 0x37, 0x00],
        InterfaceInfo {
            name: "Uniswap V2".to_string(),
            description: "addLiquidity".to_string(),
        },
    );
    m.insert(
        [0xba, 0xa2, 0xab, 0xde],
        InterfaceInfo {
            name: "Uniswap V2".to_string(),
            description: "removeLiquidity".to_string(),
        },
    );

    // Uniswap V3 Router
    m.insert(
        [0x41, 0x4b, 0xf3, 0x89],
        InterfaceInfo {
            name: "Uniswap V3".to_string(),
            description: "exactInputSingle".to_string(),
        },
    );
    m.insert(
        [0xc0, 0x4b, 0x8d, 0x59],
        InterfaceInfo {
            name: "Uniswap V3".to_string(),
            description: "exactInput".to_string(),
        },
    );
    m.insert(
        [0xdb, 0x3e, 0x21, 0x98],
        InterfaceInfo {
            name: "Uniswap V3".to_string(),
            description: "exactOutputSingle".to_string(),
        },
    );
    m.insert(
        [0xf2, 0x8c, 0x04, 0x02],
        InterfaceInfo {
            name: "Uniswap V3".to_string(),
            description: "exactOutput".to_string(),
        },
    );

    // WETH
    m.insert(
        [0xd0, 0xe3, 0x0d, 0xb0],
        InterfaceInfo {
            name: "WETH".to_string(),
            description: "deposit()".to_string(),
        },
    );
    m.insert(
        [0x2e, 0x1a, 0x7d, 0x4d],
        InterfaceInfo {
            name: "WETH".to_string(),
            description: "withdraw(uint256)".to_string(),
        },
    );

    // Aave
    m.insert(
        [0xe8, 0xed, 0xa6, 0xdf],
        InterfaceInfo {
            name: "Aave".to_string(),
            description: "deposit".to_string(),
        },
    );
    m.insert(
        [0x69, 0x32, 0x8d, 0xec],
        InterfaceInfo {
            name: "Aave".to_string(),
            description: "withdraw".to_string(),
        },
    );
    m.insert(
        [0xc6, 0x7d, 0x3a, 0x2d],
        InterfaceInfo {
            name: "Aave".to_string(),
            description: "borrow".to_string(),
        },
    );
    m.insert(
        [0x57, 0x3a, 0xdd, 0x80],
        InterfaceInfo {
            name: "Aave".to_string(),
            description: "repay".to_string(),
        },
    );

    // Compound
    m.insert(
        [0xa0, 0x71, 0x2d, 0x68],
        InterfaceInfo {
            name: "Compound".to_string(),
            description: "mint(uint256)".to_string(),
        },
    );
    m.insert(
        [0xdb, 0x00, 0x6a, 0x75],
        InterfaceInfo {
            name: "Compound".to_string(),
            description: "redeem(uint256)".to_string(),
        },
    );
    m.insert(
        [0x85, 0x2b, 0x4a, 0x5c],
        InterfaceInfo {
            name: "Compound".to_string(),
            description: "redeemUnderlying(uint256)".to_string(),
        },
    );

    // OpenZeppelin Ownable
    m.insert(
        [0x8d, 0xa5, 0xcb, 0x5b],
        InterfaceInfo {
            name: "Ownable".to_string(),
            description: "owner()".to_string(),
        },
    );
    m.insert(
        [0xf2, 0xfd, 0xe3, 0x8b],
        InterfaceInfo {
            name: "Ownable".to_string(),
            description: "transferOwnership(address)".to_string(),
        },
    );

    // Proxy
    m.insert(
        [0x36, 0x59, 0xcb, 0xfe],
        InterfaceInfo {
            name: "Proxy".to_string(),
            description: "implementation()".to_string(),
        },
    );
    m.insert(
        [0x5c, 0x60, 0xda, 0x1b],
        InterfaceInfo {
            name: "Proxy".to_string(),
            description: "upgradeTo(address)".to_string(),
        },
    );

    m
});

pub fn detect_interface(selector: &[u8]) -> Option<&'static InterfaceInfo> {
    if selector.len() < 4 {
        return None;
    }
    let mut key = [0u8; 4];
    key.copy_from_slice(&selector[..4]);
    KNOWN_INTERFACES.get(&key)
}

pub fn detect_interfaces(selectors: &[Vec<u8>]) -> Vec<&'static InterfaceInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    for sel in selectors {
        if let Some(info) = detect_interface(sel) {
            if !seen.contains(&info.name) {
                seen.insert(&info.name);
                result.push(info);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_erc20_transfer() {
        let selector = [0xa9, 0x05, 0x9c, 0xbb];
        let info = detect_interface(&selector).unwrap();
        assert_eq!(info.name, "ERC-20");
    }

    #[test]
    fn detects_erc721() {
        let selector = [0x63, 0x52, 0x21, 0x1e];
        let info = detect_interface(&selector).unwrap();
        assert_eq!(info.name, "ERC-721");
    }

    #[test]
    fn detects_uniswap_v2() {
        let selector = [0x38, 0xed, 0x17, 0x39];
        let info = detect_interface(&selector).unwrap();
        assert_eq!(info.name, "Uniswap V2");
    }

    #[test]
    fn detects_weth() {
        let selector = [0xd0, 0xe3, 0x0d, 0xb0];
        let info = detect_interface(&selector).unwrap();
        assert_eq!(info.name, "WETH");
    }

    #[test]
    fn unknown_selector_returns_none() {
        let selector = [0xff, 0xff, 0xff, 0xff];
        assert!(detect_interface(&selector).is_none());
    }

    #[test]
    fn detects_multiple_interfaces() {
        let selectors = vec![
            vec![0xa9, 0x05, 0x9c, 0xbb],
            vec![0x09, 0x5e, 0xa7, 0xb3],
            vec![0x38, 0xed, 0x17, 0x39],
        ];
        let interfaces = detect_interfaces(&selectors);
        assert!(interfaces.len() >= 2);
    }
}
