#![allow(dead_code)]

use ads_bazaar_shared::CampaignId;
use soroban_sdk::{contracttype, Address, Env, String};

use crate::error::Error;
use crate::types::{Application, Campaign};

/// Extend persistent entries by roughly this many ledgers on every write
/// (~30 days at 5s/ledger). TODO(contributors): tune once real rent/TTL
/// costs on target networks are benchmarked, and consider a max-TTL bump on
/// read-heavy paths too.
const PERSISTENT_BUMP_LEDGERS: u32 = 518_400;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = 500_000;

/// Same ~30-day-at-5s/ledger bump as `PERSISTENT_BUMP_LEDGERS`, but for
/// instance storage (admin/treasury/fee_bps/dispute_contract config keys).
/// Kept as a separate constant since instance and persistent TTL are
/// tracked independently by the ledger even when the numbers happen to match.
const INSTANCE_BUMP_LEDGERS: u32 = 518_400;
const INSTANCE_LIFETIME_THRESHOLD: u32 = 500_000;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Treasury,
    FeeBps,
    DisputeContract,
    Version,
    NextCampaignId,
    Campaign(CampaignId),
    Application(CampaignId, Address),
}

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

/// Bump the instance entry's TTL. Call this from any read-heavy path that
/// touches instance storage (config reads, not just writes) so the config
/// doesn't expire from lack of writes alone.
pub fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_LEDGERS);
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)
}

pub fn set_treasury(env: &Env, treasury: &Address) {
    env.storage().instance().set(&DataKey::Treasury, treasury);
}

pub fn get_treasury(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Treasury)
        .ok_or(Error::NotInitialized)
}

pub fn set_fee_bps(env: &Env, fee_bps: i128) {
    env.storage().instance().set(&DataKey::FeeBps, &fee_bps);
}

pub fn get_fee_bps(env: &Env) -> Result<i128, Error> {
    env.storage()
        .instance()
        .get(&DataKey::FeeBps)
        .ok_or(Error::NotInitialized)
}

pub fn set_dispute_contract(env: &Env, dispute_contract: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::DisputeContract, dispute_contract);
}

pub fn get_dispute_contract(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::DisputeContract)
        .ok_or(Error::NotInitialized)
}

pub fn set_version(env: &Env, version: &String) {
    env.storage().instance().set(&DataKey::Version, version);
}

pub fn get_version(env: &Env) -> Result<String, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Version)
        .ok_or(Error::NotInitialized)
}

pub fn next_campaign_id(env: &Env) -> CampaignId {
    let id: CampaignId = env
        .storage()
        .instance()
        .get(&DataKey::NextCampaignId)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::NextCampaignId, &(id + 1));
    id
}

pub fn get_campaign(env: &Env, id: CampaignId) -> Result<Campaign, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Campaign(id))
        .ok_or(Error::CampaignNotFound)
}

pub fn set_campaign(env: &Env, campaign: &Campaign) {
    let key = DataKey::Campaign(campaign.id);
    env.storage().persistent().set(&key, campaign);
    env.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_LEDGERS,
    );
}

pub fn get_application(
    env: &Env,
    campaign_id: CampaignId,
    creator: &Address,
) -> Result<Application, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Application(campaign_id, creator.clone()))
        .ok_or(Error::ApplicationNotFound)
}

pub fn set_application(env: &Env, application: &Application) {
    let key = DataKey::Application(application.campaign_id, application.creator.clone());
    env.storage().persistent().set(&key, application);
    env.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_LEDGERS,
    );
}
