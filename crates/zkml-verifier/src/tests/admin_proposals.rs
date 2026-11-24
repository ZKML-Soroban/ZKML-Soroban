//! The lifecycle of an admin proposal since VERSION 11: it expires after
//! `ADMIN_PROPOSAL_LEDGERS`, the admin can withdraw it, anyone can read it,
//! and `accept_admin` without a live one is a typed error, not a panic.

use super::*;
use soroban_sdk::testutils::{Address as _, Events, Ledger as _};
use soroban_sdk::{Env, IntoVal};
use test_utils::create_dummy_vk;

fn setup(env: &Env) -> (ZkmlVerifierContractClient<'_>, Address) {
    let admin = Address::generate(env);
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            admin.clone(),
            Bytes::from_slice(env, &[3u8; 32]),
            Some(create_dummy_vk(env, 5)),
        ),
    );
    (ZkmlVerifierContractClient::new(env, &contract_id), admin)
}

fn advance(env: &Env, ledgers: u32) {
    let mut ledger = env.ledger().get();
    ledger.sequence_number += ledgers;
    env.ledger().set(ledger);
}

#[test]
fn a_proposal_can_be_accepted_up_to_its_last_ledger() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let proposed = Address::generate(&env);
    let start = env.ledger().sequence();
    client.propose_admin(&proposed);

    let pending = client.get_pending_admin().expect("a live proposal");
    assert_eq!(pending.address, proposed);
    assert_eq!(pending.expires_at_ledger, start + ADMIN_PROPOSAL_LEDGERS);

    advance(&env, ADMIN_PROPOSAL_LEDGERS);
    client.accept_admin();
    assert_eq!(client.get_admin(), proposed);
    assert!(client.get_pending_admin().is_none(), "accepting clears it");
}

#[test]
fn a_proposal_expires_one_ledger_later() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    client.propose_admin(&Address::generate(&env));

    advance(&env, ADMIN_PROPOSAL_LEDGERS + 1);
    assert!(client.get_pending_admin().is_none());
    assert_eq!(
        client.try_accept_admin(),
        Err(Ok(VerificationError::PendingAdminMismatch))
    );
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn a_cancelled_proposal_cannot_be_accepted() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let proposed = Address::generate(&env);
    client.propose_admin(&proposed);

    client.cancel_admin_proposal();
    assert_eq!(
        env.events().all().filter_by_contract(&client.address),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("cfg_upd"), symbol_short!("adm_cncl")).into_val(&env),
                proposed.into_val(&env),
            ),
        ],
        "a withdrawn proposal must be visible"
    );
    assert!(client.get_pending_admin().is_none());
    assert_eq!(
        client.try_accept_admin(),
        Err(Ok(VerificationError::PendingAdminMismatch))
    );
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn cancelling_with_nothing_pending_changes_nothing() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    client.cancel_admin_proposal();
    assert_eq!(
        env.events().all().filter_by_contract(&client.address),
        vec![&env],
        "nothing was withdrawn, so nothing is published"
    );
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn a_new_proposal_replaces_the_previous_one() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let first = Address::generate(&env);
    let second = Address::generate(&env);
    client.propose_admin(&first);
    advance(&env, 100);
    client.propose_admin(&second);

    let pending = client.get_pending_admin().unwrap();
    assert_eq!(pending.address, second);
    // The window restarts with the new proposal.
    assert_eq!(
        pending.expires_at_ledger,
        env.ledger().sequence() + ADMIN_PROPOSAL_LEDGERS
    );
    client.accept_admin();
    assert_eq!(client.get_admin(), second);
}
