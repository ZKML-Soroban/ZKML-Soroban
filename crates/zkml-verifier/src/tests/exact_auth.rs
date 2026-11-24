//! Every admin entry point against exact authorizations: no authorization,
//! one signed by the wrong address, and one signed by the right address.
//!
//! The other tests deploy with `mock_all_auths`, which approves every
//! `require_auth` whoever it names, so they cannot tell a check on the admin
//! from a check on anyone, or from no check at all. These use `mock_auths`
//! with one entry per call, which is what a real transaction carries.

use super::*;
use soroban_sdk::testutils::{Address as _, MockAuth, MockAuthInvoke};
use soroban_sdk::xdr::{ScErrorCode, ScErrorType};
use soroban_sdk::{Env, IntoVal, Val};
use test_utils::create_dummy_vk;

/// What a rejected `require_auth` surfaces as through a `try_` call.
///
/// The host reports the failed authorization as an aborted invocation, so the
/// caller sees a context error rather than an auth one. That error is not
/// specific to authorization, which is why every test below repeats the same
/// call with the same arguments under the right signer and expects success:
/// the signer is the only thing that changes between the two.
fn auth_error() -> soroban_sdk::Error {
    soroban_sdk::Error::from_type_and_code(ScErrorType::Context, ScErrorCode::InvalidAction)
}

macro_rules! assert_auth_error {
    ($result:expr) => {
        match $result {
            Err(Ok(error)) => assert_eq!(error, auth_error()),
            other => panic!("expected an auth error, got {other:?}"),
        }
    };
}

/// A deployed contract, with the blanket mock dropped afterwards.
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
    env.set_auths(&[]);
    (ZkmlVerifierContractClient::new(env, &contract_id), admin)
}

/// Authorize exactly one call, `fn_name(args)` on the contract, by `signer`.
fn authorize(
    env: &Env,
    client: &ZkmlVerifierContractClient<'_>,
    signer: &Address,
    fn_name: &str,
    args: soroban_sdk::Vec<Val>,
) {
    env.mock_auths(&[MockAuth {
        address: signer,
        invoke: &MockAuthInvoke {
            contract: &client.address,
            fn_name,
            args,
            sub_invokes: &[],
        },
    }]);
}

fn config(env: &Env) -> Risc0Config {
    let mut control_id = [0u8; 32];
    control_id[0] = 1;
    Risc0Config {
        image_id: BytesN::from_array(env, &[1u8; 32]),
        control_root: BytesN::from_array(env, &[2u8; 32]),
        bn254_control_id: BytesN::from_array(env, &control_id),
        selector: BytesN::from_array(env, &[0xaa; 4]),
    }
}

#[test]
fn set_model_hash_needs_the_admin() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let stranger = Address::generate(&env);
    let new_hash = Bytes::from_slice(&env, &[5u8; 32]);
    let args: soroban_sdk::Vec<Val> = (new_hash.clone(),).into_val(&env);

    assert_auth_error!(client.try_set_model_hash(&new_hash));
    authorize(&env, &client, &stranger, "set_model_hash", args.clone());
    assert_auth_error!(client.try_set_model_hash(&new_hash));
    assert_eq!(
        client.get_model_hash(),
        Some(Bytes::from_slice(&env, &[3u8; 32]))
    );

    authorize(&env, &client, &admin, "set_model_hash", args);
    client.set_model_hash(&new_hash);
    assert_eq!(client.get_model_hash(), Some(new_hash));
}

#[test]
fn set_verification_key_needs_the_admin() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let stranger = Address::generate(&env);
    let vk = crate::test_budget::create_accept_fixture_vk(&env);
    let args: soroban_sdk::Vec<Val> = (vk.clone(),).into_val(&env);

    assert_auth_error!(client.try_set_verification_key(&vk));
    authorize(
        &env,
        &client,
        &stranger,
        "set_verification_key",
        args.clone(),
    );
    assert_auth_error!(client.try_set_verification_key(&vk));

    authorize(&env, &client, &admin, "set_verification_key", args);
    client.set_verification_key(&vk);
}

#[test]
fn set_risc0_config_needs_the_admin() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let stranger = Address::generate(&env);
    let config = config(&env);
    let args: soroban_sdk::Vec<Val> = (config.clone(),).into_val(&env);

    assert_auth_error!(client.try_set_risc0_config(&config));
    authorize(&env, &client, &stranger, "set_risc0_config", args.clone());
    assert_auth_error!(client.try_set_risc0_config(&config));

    authorize(&env, &client, &admin, "set_risc0_config", args);
    client.set_risc0_config(&config);
    assert_eq!(client.get_risc0_config().unwrap().image_id, config.image_id);
}

#[test]
fn set_risc0_vk_needs_the_admin() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let stranger = Address::generate(&env);
    let vk = create_dummy_vk(&env, 6);
    let args: soroban_sdk::Vec<Val> = (vk.clone(),).into_val(&env);

    assert_auth_error!(client.try_set_risc0_vk(&vk));
    authorize(&env, &client, &stranger, "set_risc0_vk", args.clone());
    assert_auth_error!(client.try_set_risc0_vk(&vk));

    authorize(&env, &client, &admin, "set_risc0_vk", args);
    client.set_risc0_vk(&vk);
}

#[test]
fn set_pause_needs_the_admin() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let stranger = Address::generate(&env);
    let args: soroban_sdk::Vec<Val> = (true,).into_val(&env);

    assert_auth_error!(client.try_set_pause(&true));
    authorize(&env, &client, &stranger, "set_pause", args.clone());
    assert_auth_error!(client.try_set_pause(&true));
    assert!(!client.is_paused());

    authorize(&env, &client, &admin, "set_pause", args);
    client.set_pause(&true);
    assert!(client.is_paused());
}

#[test]
fn propose_admin_needs_the_admin() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let stranger = Address::generate(&env);
    let args: soroban_sdk::Vec<Val> = (stranger.clone(),).into_val(&env);

    // A stranger cannot nominate itself.
    assert_auth_error!(client.try_propose_admin(&stranger));
    authorize(&env, &client, &stranger, "propose_admin", args.clone());
    assert_auth_error!(client.try_propose_admin(&stranger));

    authorize(&env, &client, &admin, "propose_admin", args);
    client.propose_admin(&stranger);
    assert_eq!(
        client.get_admin(),
        admin,
        "a proposal alone transfers nothing"
    );
}

#[test]
fn accept_admin_needs_the_proposed_admin() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let proposed = Address::generate(&env);
    authorize(
        &env,
        &client,
        &admin,
        "propose_admin",
        (proposed.clone(),).into_val(&env),
    );
    client.propose_admin(&proposed);
    let no_args = soroban_sdk::Vec::<Val>::new(&env);

    // accept_admin returns Result<(), VerificationError>, and a failed
    // authorization is not one of its errors, so it surfaces as an invoke
    // error rather than through the contract's error type.
    let refused = |result: Result<_, Result<VerificationError, soroban_sdk::InvokeError>>| {
        matches!(result, Err(Err(_)))
    };
    assert!(refused(client.try_accept_admin()));
    // Not even the current admin can accept on the proposed admin's behalf.
    authorize(&env, &client, &admin, "accept_admin", no_args.clone());
    assert!(refused(client.try_accept_admin()));
    assert_eq!(client.get_admin(), admin);

    authorize(&env, &client, &proposed, "accept_admin", no_args);
    client.accept_admin();
    assert_eq!(client.get_admin(), proposed);
}
