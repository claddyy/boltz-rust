pub mod e;
pub mod boltz;
pub mod config;
pub mod seed;
pub mod derivation;
pub mod ec;
pub mod util;
pub mod script;
pub mod address;
pub mod sync;
pub mod electrum;
pub mod tx;

#[cfg(test)]
mod tests {
    use std::{env, str::FromStr};
    use bitcoin::Network;
    use electrum_client::ElectrumApi;
    use secp256k1::{rand::{thread_rng, Rng}, hashes::ripemd160};
    use crate::{config::WalletConfig, address, seed::import, derivation::{to_hardened_account, DerivationPurpose}, ec::{keypair_from_xprv_str, KeyPairString}, util::rnd_str, boltz::{BoltzApiClient, CreateSwapRequest, SwapType, PairId, OrderSide, SwapStatusRequest, BOLTZ_TESTNET_URL}, script::{ SwapRedeemScriptElements, self, ReverseSwapRedeemScriptElements, }, electrum::{NetworkConfig, BitcoinNetwork, DEFAULT_TESTNET_NODE}};
    use dotenv::dotenv;
    use bitcoin::hashes::{sha256, Hash};

    use std::io;
    use std::io::prelude::*;

    fn pause_and_wait() {
        let mut stdin = io::stdin();
        let mut stdout = io::stdout();
        write!(stdout, "Press Enter to continue...").unwrap();
        stdout.flush().unwrap();
        let _ = stdin.read_line(&mut String::new()).unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_rsi() {
        const RETURN_ADDRESS: &str = "tb1qw2c3lxufxqe2x9s4rdzh65tpf4d7fssjgh8nv6";
        dotenv().ok();
        let mnemonic = match env::var("MNEMONIC") {
            Ok(result) => result,
            Err(e) => panic!("Couldn't read MNEMONIC ({})", e),
        };
        println!("{}", mnemonic);
        let master_key = import(&mnemonic, "" , Network::Testnet).unwrap();
        let child_key = to_hardened_account(&master_key.xprv, DerivationPurpose::Native, 0).unwrap();
        let ec_key = keypair_from_xprv_str(&child_key.xprv).unwrap();
        let string_keypair = KeyPairString::from_keypair(ec_key);
        println!("{:?}",string_keypair);
        let preimage = rnd_str();
        println!("Preimage: {:?}", preimage);
        let preimage_hash =  sha256::Hash::hash(&hex::decode(preimage).unwrap());

        let network_config = NetworkConfig::new(
            BitcoinNetwork::BitcoinTestnet,
            DEFAULT_TESTNET_NODE,
            true,
            true,
            false,
            None,
        ).unwrap();
        let electrum_client = network_config.electrum_url.build_client().unwrap();
        let boltz_client = BoltzApiClient::new(BOLTZ_TESTNET_URL);
       
        let boltz_pairs = boltz_client.get_pairs().await.unwrap();
        
        let pair_hash = boltz_pairs.pairs.pairs.get("BTC/BTC")
            .map(|pair_info| pair_info.hash.clone())
            .unwrap();
        let timeout: u32 = 3_989_055;
        /*
         * 
         * 
         * 
         * TIMEOUT NEEDS TO BE CLARIFIED
         * SET BY BOLTZ
         * 
         * 
         * 
         */

        let request = CreateSwapRequest::new_reverse(
            SwapType::ReverseSubmarine, 
            PairId::Btc_Btc, 
            OrderSide::Buy, 
            pair_hash, 
            preimage_hash.to_string(), 
            string_keypair.pubkey.clone(), 
            timeout as u64,
            100_000
        );
        let response = boltz_client.create_swap(request).await;
        assert!(response.is_ok());
        println!("{}",preimage_hash.to_string());
        assert!(response.as_ref().unwrap().validate_preimage(preimage_hash.to_string()));
        // assert_eq!(timeout as u64 , response.as_ref().unwrap().timeout_block_height.unwrap().clone());

        let timeout = response.as_ref().unwrap().timeout_block_height.unwrap().clone();
        let id = response.as_ref().unwrap().id.as_str().clone();
        let invoice = response.as_ref().unwrap().invoice.clone().unwrap();
        let lockup_address = response.as_ref().unwrap().lockup_address.clone().unwrap();

        let boltz_script_elements = ReverseSwapRedeemScriptElements::from_str(&response.as_ref().unwrap().redeem_script.as_ref().unwrap().clone()).unwrap();
        // assert!(response.as_ref().unwrap().claim_public_key.as_ref().unwrap().clone() == boltz_script_elements.sender_pubkey);
        let hash160 = ripemd160::Hash::hash(&hex::decode(preimage_hash.to_string()).unwrap());
        let constructed_script_elements = ReverseSwapRedeemScriptElements{
            hashlock: hash160.to_string(),
            reciever_pubkey: string_keypair.pubkey.clone(),
            timelock: timeout as u32,
            sender_pubkey: boltz_script_elements.sender_pubkey.clone(),
        };
        println!("{:?} , {:?}", constructed_script_elements, boltz_script_elements);

        assert!(constructed_script_elements == boltz_script_elements);
        let constructed_address = constructed_script_elements.to_address(Network::Testnet);
        println!("{}", constructed_address.to_string());
        assert!(constructed_address.to_string() == lockup_address);

        let script_balance = electrum_client.script_get_balance(&constructed_script_elements.to_script()).unwrap();
        assert_eq!(script_balance.unconfirmed, 0);
        assert_eq!(script_balance.confirmed, 0);
        println!("*******PAY********************");
        println!("*******LN*********************");
        println!("*******INVOICE****************");
        println!("{}",invoice);
        println!("");
        println!("Once you have paid the invoice, press enter to continue the tests.");
        println!("******************************");

        loop{
            pause_and_wait();
            let request = SwapStatusRequest{id: id.to_string()};
            let response = boltz_client.swap_status(request).await;
            assert!(response.is_ok());
            let swap_status = response.unwrap().status;
            if swap_status == "swap.created"{
                println!("Your turn: Pay the invoice");

            }
            if swap_status == "transaction.mempool"{
                println!("*******BOLTZ******************");
                println!("*******ONCHAIN-TX*************");
                println!("*******DETECTED***************");
            }
            if swap_status == "transaction.confirmed"{
                println!("*******BOLTZ******************");
                println!("*******ONCHAIN-TX*************");
                println!("*******CONFIRMED**************");
                break
            }
        }
        assert!(false);

    }
    
}