//! Baseline tests covering what's actually implemented so far
//! (`initialize` and the read-only getters). As contributors fill in the
//! `todo!()` bodies in `lib.rs`, add corresponding tests here — e.g.
//! `test_create_and_fund_campaign`, `test_release_payment_pays_creator_minus_fee`.
#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{BytesN, Env};

fn setup(env: &Env) -> (CampaignEscrowContractClient<'_>, Address, Address) {
    let contract_id = env.register(CampaignEscrowContract, ());
    let client = CampaignEscrowContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let dispute_contract = Address::generate(env);
    (client, admin, dispute_contract)
}

#[test]
fn initialize_sets_admin_and_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);

    client.initialize(&admin, &dispute_contract, &250);
}

#[test]
fn initialize_twice_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);

    client.initialize(&admin, &dispute_contract, &250);
    let result = client.try_initialize(&admin, &dispute_contract, &250);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn initialize_rejects_out_of_range_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);

    let result = client.try_initialize(
        &admin,
        &dispute_contract,
        &(ads_bazaar_shared::BASIS_POINTS_DENOMINATOR + 1),
    );
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn get_protocol_config_returns_current_fee_bps() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);
    client.initialize(&admin, &dispute_contract, &150);

    let config = client.get_protocol_config();
    assert_eq!(config.fee_bps, 150);
    assert_eq!(config.admin, admin);
    // treasury defaults to admin — see the comment on `initialize` in lib.rs
    assert_eq!(config.treasury, admin);
}

#[test]
fn get_protocol_config_fails_before_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _dispute_contract) = setup(&env);

    let result = client.try_get_protocol_config();
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

#[test]
fn get_campaign_not_found_before_creation() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);
    client.initialize(&admin, &dispute_contract, &250);

    let result = client.try_get_campaign(&0);
    assert_eq!(result, Err(Ok(Error::CampaignNotFound)));
}

#[test]
fn version_returns_initial_version_after_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);
    client.initialize(&admin, &dispute_contract, &250);

    assert_eq!(client.version(), String::from_str(&env, "0.1.0"));
}

#[test]
fn version_fails_before_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _dispute_contract) = setup(&env);

    let result = client.try_version();
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

#[test]
fn upgrade_rejects_non_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);
    client.initialize(&admin, &dispute_contract, &250);

    let not_admin = Address::generate(&env);
    let new_wasm_hash = BytesN::from_array(&env, &[7u8; 32]);
    let result = client.try_upgrade(&not_admin, &new_wasm_hash);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
#[should_panic(expected = "not yet implemented")]
fn create_campaign_is_not_yet_implemented() {
    // Documents current scaffold state: this will start failing (in a good
    // way) once `create_campaign` is implemented — replace this test with a
    // real assertion at that point.
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);
    client.initialize(&admin, &dispute_contract, &250);

    let business = Address::generate(&env);
    let token = Address::generate(&env);
    let asset = ads_bazaar_shared::PayoutAsset {
        token,
        symbol: String::from_str(&env, "USDC"),
    };

    client.create_campaign(
        &business,
        &asset,
        &1_000_000,
        &5,
        &(env.ledger().timestamp() + 86_400),
        &(env.ledger().timestamp() + 604_800),
        &String::from_str(&env, "ipfs://brief"),
    );
}

#[test]
fn pause_blocks_apply_to_campaign() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);
    client.initialize(&admin, &dispute_contract, &250);
    client.pause(&admin);
    assert!(client.is_paused());

    let creator = Address::generate(&env);
    let result =
        client.try_apply_to_campaign(&creator, &0, &String::from_str(&env, "ipfs://pitch"));
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
}

#[test]
#[should_panic(expected = "not yet implemented")]
fn unpause_allows_call_past_guard() {
    // Confirms unpause removes the pause guard: the call now reaches the
    // still-unimplemented body (`todo!()`) instead of being blocked with
    // `ContractPaused`. Replace with a real assertion once
    // `apply_to_campaign` is implemented.
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);
    client.initialize(&admin, &dispute_contract, &250);

    let business = Address::generate(&env);
    let malicious = Address::generate(&env);
    let token_addr = Address::generate(&env);

    let asset = ads_bazaar_shared::PayoutAsset {
        token: token_addr,
        symbol: String::from_str(&env, "USDC"),
    };

    let campaign_id = 1;
    let campaign = Campaign {
        id: campaign_id,
        business: business.clone(),
        asset,
        total_budget: 1_000_000,
        escrow_balance: 1_000_000,
        max_creators: 5,
        approved_count: 0,
        application_deadline: env.ledger().timestamp() + 86_400,
        completion_deadline: env.ledger().timestamp() + 604_800,
        metadata_uri: String::from_str(&env, "ipfs://brief"),
        status: ads_bazaar_shared::CampaignStatus::Funded,
    };

    env.as_contract(&client.address, || {
        storage::set_campaign(&env, &campaign);
    });

    let res = client.try_cancel_campaign(&malicious, &campaign_id);
    assert_eq!(res, Err(Ok(Error::NotCampaignOwner)));
}

#[test]
fn test_cancel_draft_campaign_success() {
    client.pause(&admin);
    client.unpause(&admin);
    assert!(!client.is_paused());

    let creator = Address::generate(&env);
    client.apply_to_campaign(&creator, &0, &String::from_str(&env, "ipfs://pitch"));
}

#[test]
fn view_functions_readable_while_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);
    client.initialize(&admin, &dispute_contract, &250);
    client.pause(&admin);

    let config = client.get_protocol_config();
    assert_eq!(config.admin, admin);

    let result = client.try_get_campaign(&0);
    assert_eq!(result, Err(Ok(Error::CampaignNotFound)));

    assert!(client.is_paused());
}

#[test]
fn non_admin_cannot_pause() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);
    client.initialize(&admin, &dispute_contract, &250);

    let business = Address::generate(&env);
    let token_addr = Address::generate(&env);
    let asset = ads_bazaar_shared::PayoutAsset {
        token: token_addr,
        symbol: String::from_str(&env, "USDC"),
    };

    let campaign_id = 1;
    let campaign = Campaign {
        id: campaign_id,
        business: business.clone(),
        asset,
        total_budget: 1_000_000,
        escrow_balance: 0,
        max_creators: 5,
        approved_count: 0,
        application_deadline: env.ledger().timestamp() + 86_400,
        completion_deadline: env.ledger().timestamp() + 604_800,
        metadata_uri: String::from_str(&env, "ipfs://brief"),
        status: ads_bazaar_shared::CampaignStatus::Draft,
    };

    env.as_contract(&client.address, || {
        storage::set_campaign(&env, &campaign);
    });

    client.cancel_campaign(&business, &campaign_id);

    let updated_campaign = client.get_campaign(&campaign_id);
    assert_eq!(
        updated_campaign.status,
        ads_bazaar_shared::CampaignStatus::Cancelled
    );
    assert_eq!(updated_campaign.escrow_balance, 0);
    let not_admin = Address::generate(&env);
    let result = client.try_pause(&not_admin);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn non_admin_cannot_unpause() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);
    client.initialize(&admin, &dispute_contract, &250);
    client.pause(&admin);

    let not_admin = Address::generate(&env);
    let result = client.try_unpause(&not_admin);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn pause_unpause_toggles_is_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, dispute_contract) = setup(&env);
    client.initialize(&admin, &dispute_contract, &250);

    assert!(!client.is_paused());
    client.pause(&admin);
    assert!(client.is_paused());
    client.unpause(&admin);
    assert!(!client.is_paused());
}
