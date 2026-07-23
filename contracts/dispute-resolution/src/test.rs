//! Tests for the dispute-resolution contract.
//!
//! `raise_dispute` reads from and writes to `campaign-escrow` across a
//! contract boundary, so these tests register a real escrow contract rather
//! than a generated placeholder address and point the two at each other the
//! way a deployment would. Helpers live in `test_helpers`.
#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{BytesN, Env};

mod test_helpers {
    use crate::{DisputeResolutionContract, DisputeResolutionContractClient};
    use ads_bazaar_campaign_escrow::{CampaignEscrowContract, CampaignEscrowContractClient};
    use ads_bazaar_shared::PayoutAsset;
    use soroban_sdk::testutils::{Address as _, Ledger as _};
    use soroban_sdk::token::StellarAssetClient;
    use soroban_sdk::{Address, Env, String};

    pub const BASE_TIME: u64 = 1_000_000;
    pub const BUSINESS_FUNDS: i128 = 1_000_000_000;
    pub const PAYOUT: i128 = 1_000_000;
    pub const COMPLETION_WINDOW: u64 = 604_800;

    /// Register both contracts at a fixed base timestamp with all auths
    /// mocked. Returns `(env, escrow_id, dispute_id)`.
    pub fn setup_env() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = BASE_TIME);
        let escrow_id = env.register(CampaignEscrowContract, ());
        let dispute_id = env.register(DisputeResolutionContract, ());
        (env, escrow_id, dispute_id)
    }

    pub struct Fixture<'a> {
        pub escrow: CampaignEscrowContractClient<'a>,
        pub disputes: DisputeResolutionContractClient<'a>,
        pub business: Address,
        pub creator: Address,
        pub campaign_id: u64,
    }

    impl Fixture<'_> {
        /// Take another creator through apply → approve → submit proof on the
        /// same campaign, so tests can assert on a second disputable payout.
        pub fn add_creator(&self, env: &Env) -> Address {
            let creator = Address::generate(env);
            self.escrow.apply_to_campaign(
                &creator,
                &self.campaign_id,
                &String::from_str(env, "pitch"),
            );
            self.escrow
                .approve_creator(&self.business, &self.campaign_id, &creator, &PAYOUT);
            self.escrow
                .submit_proof(&creator, &self.campaign_id, &String::from_str(env, "proof"));
            creator
        }
    }

    /// Initialize both contracts pointing at each other, fund a campaign, and
    /// take one creator as far as a submitted (not yet approved) proof — the
    /// state a dispute is actually raised from.
    pub fn bootstrap<'a>(env: &'a Env, escrow_id: &Address, dispute_id: &Address) -> Fixture<'a> {
        let escrow = CampaignEscrowContractClient::new(env, escrow_id);
        let disputes = DisputeResolutionContractClient::new(env, dispute_id);
        let admin = Address::generate(env);
        escrow.initialize(&admin, dispute_id, &50);
        disputes.initialize(&admin, escrow_id);

        let business = Address::generate(env);
        let token = env.register_stellar_asset_contract_v2(Address::generate(env));
        StellarAssetClient::new(env, &token.address()).mint(&business, &BUSINESS_FUNDS);

        let now = env.ledger().timestamp();
        let campaign_id = escrow.create_campaign(
            &business,
            &PayoutAsset {
                token: token.address(),
                symbol: String::from_str(env, "USDC"),
            },
            &10_000_000,
            &5,
            &(now + 86_400),
            &(now + COMPLETION_WINDOW),
            &String::from_str(env, "ipfs://brief"),
        );
        escrow.fund_campaign(&business, &campaign_id);

        let fixture = Fixture {
            escrow,
            disputes,
            business,
            creator: Address::generate(env),
            campaign_id,
        };
        let creator = fixture.add_creator(env);
        Fixture { creator, ..fixture }
    }
}

fn setup(env: &Env) -> (DisputeResolutionContractClient<'_>, Address, Address) {
    let contract_id = env.register(DisputeResolutionContract, ());
    let client = DisputeResolutionContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let escrow_contract = Address::generate(env);
    (client, admin, escrow_contract)
}

#[test]
fn initialize_sets_admin_and_escrow_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, escrow_contract) = setup(&env);

    client.initialize(&admin, &escrow_contract);
}

#[test]
fn initialize_twice_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, escrow_contract) = setup(&env);

    client.initialize(&admin, &escrow_contract);
    let result = client.try_initialize(&admin, &escrow_contract);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn version_returns_initial_version_after_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, escrow_contract) = setup(&env);
    client.initialize(&admin, &escrow_contract);

    assert_eq!(client.version(), String::from_str(&env, "0.1.0"));
}

#[test]
fn version_fails_before_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _escrow_contract) = setup(&env);

    let result = client.try_version();
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

#[test]
fn upgrade_rejects_non_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, escrow_contract) = setup(&env);
    client.initialize(&admin, &escrow_contract);

    let not_admin = Address::generate(&env);
    let new_wasm_hash = BytesN::from_array(&env, &[7u8; 32]);
    let result = client.try_upgrade(&not_admin, &new_wasm_hash);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn get_dispute_not_found_before_creation() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, escrow_contract) = setup(&env);
    client.initialize(&admin, &escrow_contract);

    let result = client.try_get_dispute(&0);
    assert_eq!(result, Err(Ok(Error::DisputeNotFound)));
}

mod test_raise_dispute {
    use super::test_helpers::*;
    use crate::Error;
    use ads_bazaar_shared::{DisputeOutcome, DisputeStatus};
    use soroban_sdk::testutils::{Address as _, Ledger as _};
    use soroban_sdk::{Address, String};

    #[test]
    fn creator_raises_dispute_and_stored_fields_match() {
        let (env, escrow_id, dispute_id) = setup_env();
        let f = bootstrap(&env, &escrow_id, &dispute_id);
        let reason = String::from_str(&env, "ipfs://evidence");

        let id = f
            .disputes
            .raise_dispute(&f.creator, &f.campaign_id, &f.creator, &reason);
        assert_eq!(id, 0);

        let dispute = f.disputes.get_dispute(&id);
        assert_eq!(dispute.campaign_id, f.campaign_id);
        assert_eq!(dispute.creator, f.creator);
        assert_eq!(dispute.raised_by, f.creator);
        assert_eq!(dispute.reason_uri, reason);
        assert_eq!(dispute.arbiter, None);
        assert_eq!(dispute.status, DisputeStatus::Raised);
        assert_eq!(dispute.outcome, DisputeOutcome::Pending);
        assert_eq!(dispute.raised_at, BASE_TIME);
        assert_eq!(dispute.resolved_at, None);
    }

    #[test]
    fn dispute_ids_increment_per_payout() {
        let (env, escrow_id, dispute_id) = setup_env();
        let f = bootstrap(&env, &escrow_id, &dispute_id);
        let reason = String::from_str(&env, "ipfs://evidence");
        let second = f.add_creator(&env);

        let first_id = f
            .disputes
            .raise_dispute(&f.creator, &f.campaign_id, &f.creator, &reason);
        let second_id = f
            .disputes
            .raise_dispute(&second, &f.campaign_id, &second, &reason);

        assert_eq!(first_id, 0);
        assert_eq!(second_id, 1);
        assert_eq!(f.disputes.get_dispute(&second_id).creator, second);
    }

    #[test]
    fn business_may_raise_dispute_against_creator() {
        let (env, escrow_id, dispute_id) = setup_env();
        let f = bootstrap(&env, &escrow_id, &dispute_id);

        let id = f.disputes.raise_dispute(
            &f.business,
            &f.campaign_id,
            &f.creator,
            &String::from_str(&env, "ipfs://not-as-briefed"),
        );

        let dispute = f.disputes.get_dispute(&id);
        assert_eq!(dispute.raised_by, f.business);
        assert_eq!(dispute.creator, f.creator);
    }

    #[test]
    fn stranger_cannot_raise_dispute() {
        let (env, escrow_id, dispute_id) = setup_env();
        let f = bootstrap(&env, &escrow_id, &dispute_id);

        let stranger = Address::generate(&env);
        let result = f.disputes.try_raise_dispute(
            &stranger,
            &f.campaign_id,
            &f.creator,
            &String::from_str(&env, "ipfs://evidence"),
        );

        assert_eq!(result, Err(Ok(Error::Unauthorized)));
        // A rejected raise must not have frozen the payout on its way out.
        assert!(!f.escrow.get_application(&f.campaign_id, &f.creator).frozen);
    }

    #[test]
    fn raise_dispute_freezes_payout_in_escrow() {
        let (env, escrow_id, dispute_id) = setup_env();
        let f = bootstrap(&env, &escrow_id, &dispute_id);
        f.escrow
            .approve_submission(&f.business, &f.campaign_id, &f.creator);

        f.disputes.raise_dispute(
            &f.creator,
            &f.campaign_id,
            &f.creator,
            &String::from_str(&env, "ipfs://evidence"),
        );

        assert!(f.escrow.get_application(&f.campaign_id, &f.creator).frozen);
        assert_eq!(
            f.escrow.try_claim_payment(&f.creator, &f.campaign_id),
            Err(Ok(ads_bazaar_campaign_escrow::Error::PayoutFrozen))
        );
    }

    #[test]
    fn second_dispute_over_same_payout_rejected() {
        let (env, escrow_id, dispute_id) = setup_env();
        let f = bootstrap(&env, &escrow_id, &dispute_id);
        let reason = String::from_str(&env, "ipfs://evidence");

        f.disputes
            .raise_dispute(&f.creator, &f.campaign_id, &f.creator, &reason);
        let result = f
            .disputes
            .try_raise_dispute(&f.business, &f.campaign_id, &f.creator, &reason);

        assert_eq!(result, Err(Ok(Error::DisputeAlreadyRaised)));
    }

    #[test]
    fn empty_reason_rejected() {
        let (env, escrow_id, dispute_id) = setup_env();
        let f = bootstrap(&env, &escrow_id, &dispute_id);

        let result = f.disputes.try_raise_dispute(
            &f.creator,
            &f.campaign_id,
            &f.creator,
            &String::from_str(&env, ""),
        );

        assert_eq!(result, Err(Ok(Error::InvalidReason)));
    }

    #[test]
    fn dispute_may_be_raised_after_content_deadline() {
        let (env, escrow_id, dispute_id) = setup_env();
        let f = bootstrap(&env, &escrow_id, &dispute_id);

        // Past the deadline the creator is auto-approved and could claim at
        // any moment — exactly when a business needs to be able to dispute.
        env.ledger()
            .with_mut(|l| l.timestamp = BASE_TIME + COMPLETION_WINDOW + 1);
        f.disputes.raise_dispute(
            &f.business,
            &f.campaign_id,
            &f.creator,
            &String::from_str(&env, "ipfs://evidence"),
        );

        assert_eq!(
            f.escrow.try_claim_payment(&f.creator, &f.campaign_id),
            Err(Ok(ads_bazaar_campaign_escrow::Error::PayoutFrozen))
        );
    }

    #[test]
    fn dispute_over_already_paid_payout_is_rejected_by_escrow() {
        let (env, escrow_id, dispute_id) = setup_env();
        let f = bootstrap(&env, &escrow_id, &dispute_id);
        f.escrow
            .approve_submission(&f.business, &f.campaign_id, &f.creator);
        f.escrow.claim_payment(&f.creator, &f.campaign_id);

        let result = f.disputes.try_raise_dispute(
            &f.creator,
            &f.campaign_id,
            &f.creator,
            &String::from_str(&env, "ipfs://too-late"),
        );

        assert!(result.is_err());
        assert_eq!(
            f.disputes.try_get_dispute(&0),
            Err(Ok(Error::DisputeNotFound))
        );
    }

    #[test]
    fn creator_with_no_application_cannot_raise_dispute() {
        let (env, escrow_id, dispute_id) = setup_env();
        let f = bootstrap(&env, &escrow_id, &dispute_id);

        let stranger = Address::generate(&env);
        let result = f.disputes.try_raise_dispute(
            &stranger,
            &f.campaign_id,
            &stranger,
            &String::from_str(&env, "ipfs://evidence"),
        );

        assert!(result.is_err());
    }

    #[test]
    fn other_creators_stay_claimable_while_one_is_disputed() {
        let (env, escrow_id, dispute_id) = setup_env();
        let f = bootstrap(&env, &escrow_id, &dispute_id);
        let uncontested = f.add_creator(&env);
        f.escrow
            .approve_submission(&f.business, &f.campaign_id, &uncontested);

        f.disputes.raise_dispute(
            &f.creator,
            &f.campaign_id,
            &f.creator,
            &String::from_str(&env, "ipfs://evidence"),
        );

        f.escrow.claim_payment(&uncontested, &f.campaign_id);
        assert_eq!(
            f.escrow
                .get_application(&f.campaign_id, &uncontested)
                .status,
            ads_bazaar_shared::ApplicationStatus::Paid
        );
    }
}
