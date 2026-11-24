//! Every admin setter has to leave a trace on chain.
//!
//! `log!` is not that trace: the `contract` profile sets
//! `debug-assertions = false`, so in the deployed WASM the log lines do not
//! exist. The threat model accepts a malicious admin on the grounds that the
//! change is *detectable*, and that only holds if each setter emits an event.
//! These tests are what keeps that argument true.

use super::*;
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{Env, IntoVal};

fn setup(env: &Env) -> ZkmlVerifierContractClient<'_> {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let model_hash = Bytes::from_slice(env, &[7u8; 32]);
    let vk = crate::test_utils::create_dummy_vk(env, 5);
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash.clone(), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(env, &contract_id);
    client
}

#[test]
fn set_verification_key_emits_an_event() {
    let env = Env::default();
    let client = setup(&env);
    let vk = crate::test_utils::create_dummy_vk(&env, 5);

    client.set_verification_key(&vk);

    assert_eq!(
        env.events().all().filter_by_contract(&client.address),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("cfg_upd"), symbol_short!("vk")).into_val(&env),
                ().into_val(&env),
            ),
        ],
        "changing the key verify_inference checks against must be observable"
    );
}

#[test]
fn set_model_hash_emits_the_new_commitment() {
    let env = Env::default();
    let client = setup(&env);
    let new_hash = Bytes::from_slice(&env, &[9u8; 32]);

    client.set_model_hash(&new_hash);

    assert_eq!(
        env.events().all().filter_by_contract(&client.address),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("cfg_upd"), symbol_short!("mdl_hash")).into_val(&env),
                new_hash.into_val(&env),
            ),
        ],
        "the event carries the commitment now in force"
    );
}

#[test]
fn admin_transfer_emits_propose_then_accept() {
    let env = Env::default();
    let client = setup(&env);
    let new_admin = Address::generate(&env);

    // Proposing emits adm_prop with the proposed address.
    client.propose_admin(&new_admin);
    assert_eq!(
        env.events().all().filter_by_contract(&client.address),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("cfg_upd"), symbol_short!("adm_prop")).into_val(&env),
                new_admin.clone().into_val(&env),
            ),
        ],
        "a proposed transfer must be visible"
    );

    // Accepting emits admin with the address that now controls the contract.
    client.accept_admin();
    assert_eq!(
        env.events().all().filter_by_contract(&client.address),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("cfg_upd"), symbol_short!("admin")).into_val(&env),
                new_admin.into_val(&env),
            ),
        ],
        "the completed handover is the change that most needs a record"
    );
}

/// Both directions, asserted one call at a time.
///
/// `env.events().all()` holds only what the last invocation published, not a
/// running log, so the two calls have to be checked separately.
#[test]
fn set_pause_emits_the_flag_both_ways() {
    let env = Env::default();
    let client = setup(&env);

    for paused in [true, false] {
        client.set_pause(&paused);
        assert_eq!(
            env.events().all().filter_by_contract(&client.address),
            vec![
                &env,
                (
                    client.address.clone(),
                    (symbol_short!("cfg_upd"), symbol_short!("pause")).into_val(&env),
                    paused.into_val(&env),
                ),
            ],
            "set_pause({paused}) must leave a record"
        );
    }
}
