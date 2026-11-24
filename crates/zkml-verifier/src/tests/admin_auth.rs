use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Env;
use test_utils::create_dummy_vk;

fn setup_with_admin(env: &Env) -> (ZkmlVerifierContractClient<'_>, Address) {
    let admin = Address::generate(env);
    let model_hash = Bytes::from_slice(env, &[3u8; 32]);
    let vk = create_dummy_vk(env, 5);
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash.clone(), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(env, &contract_id);
    (client, admin)
}

#[test]
fn constructor_requires_admin_auth() {
    // The constructor calls `admin.require_auth()`, so deploying demands the
    // admin's authorization, recorded in `env.auths()` against the admin
    // address. That requirement is what stops a third party from deploying
    // under an admin they do not control, which is the front-running the
    // old public `initialize` allowed.
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let model_hash = Bytes::from_slice(&env, &[1u8; 32]);
    let vk = create_dummy_vk(&env, 5);

    let _ = env.register(ZkmlVerifierContract, (admin.clone(), model_hash, Some(vk)));

    assert!(
        env.auths().iter().any(|(addr, _)| addr == &admin),
        "the constructor must require the admin's authorization"
    );
}

#[test]
fn constructor_with_admin_auth_succeeds() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let model_hash = Bytes::from_slice(&env, &[1u8; 32]);
    let vk = create_dummy_vk(&env, 5);

    env.mock_all_auths();
    let contract_id = env.register(ZkmlVerifierContract, (admin.clone(), model_hash, Some(vk)));
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);

    // The admin passed at deploy time is the one stored.
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn set_verification_key_authorized_succeeds() {
    let env = Env::default();
    let (client, _admin) = setup_with_admin(&env);
    let new_vk = create_dummy_vk(&env, 5);

    // Mock auth for admin
    env.mock_all_auths();
    client.set_verification_key(&new_vk);

    // Verify contract state is still accessible
    let stored_model_hash = client.get_model_hash();
    assert_eq!(stored_model_hash, Some(Bytes::from_slice(&env, &[3u8; 32])));
}

#[test]
fn set_model_hash_authorized_succeeds() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let model_hash = Bytes::from_slice(&env, &[3u8; 32]);
    let vk = create_dummy_vk(&env, 5);

    // Initialize with auth
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash.clone(), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);

    let new_model_hash = Bytes::from_slice(&env, &[5u8; 32]);

    // Mock auth for admin again
    env.mock_all_auths();
    client.set_model_hash(&new_model_hash);

    // Verify model hash was updated
    let stored_model_hash = client.get_model_hash();
    assert_eq!(stored_model_hash, Some(new_model_hash));
}

/// `set_model_hash` without the admin's authorization fails and leaves the
/// commitment alone. This used to call `verify_inference` with a wrong model
/// hash and never touched `set_model_hash`; `test_exact_auth` now covers every
/// admin entry point against the right signer, a wrong one and none.
#[test]
fn set_model_hash_unauthorized_fails() {
    let env = Env::default();
    let (client, _admin) = setup_with_admin(&env);

    // Drop the blanket mock: from here on every require_auth needs a real
    // authorization entry, and none is given.
    env.set_auths(&[]);
    assert!(client
        .try_set_model_hash(&Bytes::from_slice(&env, &[5u8; 32]))
        .is_err());
    assert_eq!(
        client.get_model_hash(),
        Some(Bytes::from_slice(&env, &[3u8; 32]))
    );
}

#[test]
fn admin_transfer_is_two_step() {
    let env = Env::default();
    let (client, old_admin) = setup_with_admin(&env);
    let new_admin = Address::generate(&env);

    // Proposing does not change the admin yet.
    client.propose_admin(&new_admin);
    assert_eq!(
        client.get_admin(),
        old_admin,
        "proposal alone must not transfer"
    );

    // Accepting completes the transfer.
    client.accept_admin();
    assert_eq!(
        client.get_admin(),
        new_admin,
        "acceptance completes the transfer"
    );
}

#[test]
fn accept_admin_without_a_proposal_fails() {
    // Up to VERSION 10 this panicked with "no pending admin transfer".
    let env = Env::default();
    let (client, _admin) = setup_with_admin(&env);
    assert_eq!(
        client.try_accept_admin(),
        Err(Ok(VerificationError::PendingAdminMismatch))
    );
}

#[test]
fn contract_state_accessible_after_key_rotation() {
    let env = Env::default();
    let (client, _admin) = setup_with_admin(&env);

    // Rotate verification key
    let new_vk = create_dummy_vk(&env, 5);
    env.mock_all_auths();
    client.set_verification_key(&new_vk);

    // Verify that contract state is still accessible after rotation
    let stored_model_hash = client.get_model_hash();
    assert_eq!(stored_model_hash, Some(Bytes::from_slice(&env, &[3u8; 32])));
}

#[test]
fn get_admin_returns_stored_admin() {
    let env = Env::default();
    let (client, admin) = setup_with_admin(&env);

    let retrieved_admin = client.get_admin();
    assert_eq!(retrieved_admin, admin);
}

#[test]
fn is_paused_returns_correct_state() {
    let env = Env::default();
    let (client, _admin) = setup_with_admin(&env);

    // Initially not paused
    assert!(!client.is_paused());

    // Pause
    env.mock_all_auths();
    client.set_pause(&true);
    assert!(client.is_paused());

    // Unpause
    client.set_pause(&false);
    assert!(!client.is_paused());
}
