use fil_actor_datacap::{
    DestroyParams, Method as DataCapMethod, MintParams, State as DataCapState,
};
use fil_actor_verifreg::{
    AddVerifierClientParams, DataCap, RemoveDataCapParams, RemoveDataCapRequest,
    RemoveDataCapReturn, SIGNATURE_DOMAIN_SEPARATION_REMOVE_DATA_CAP,
};
use fil_actor_verifreg::{AddrPairKey, Method as VerifregMethod};
use fil_actor_verifreg::{RemoveDataCapProposal, RemoveDataCapProposalID, State as VerifregState};
use fil_actors_runtime::cbor::serialize;
use fil_actors_runtime::{
    make_map_with_root_and_bitwidth, DATACAP_TOKEN_ACTOR_ADDR, VERIFIED_REGISTRY_ACTOR_ADDR,
};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::to_vec;
use fvm_shared::bigint::Zero;
use fvm_shared::crypto::signature::{Signature, SignatureType};
use fvm_shared::econ::TokenAmount;
use fvm_shared::sector::StoragePower;
use fvm_shared::HAMT_BIT_WIDTH;
use std::ops::Sub;
use test_vm::util::{add_verifier, apply_ok, create_accounts};
use test_vm::{ExpectInvocation, TEST_VERIFREG_ROOT_ADDR, VM};

#[test]
fn remove_datacap_simple_successful_path() {
    let store = MemoryBlockstore::new();
    let v = VM::new_with_singletons(&store);
    let addrs = create_accounts(&v, 4, TokenAmount::from(10_000e18 as i128));
    let (verifier1, verifier2, verified_client) = (addrs[0], addrs[1], addrs[2]);

    let verifier1_id_addr = v.normalize_address(&verifier1).unwrap();
    let verifier2_id_addr = v.normalize_address(&verifier2).unwrap();
    let verified_client_id_addr = v.normalize_address(&verified_client).unwrap();
    let verifier_allowance = DataCap::from(StoragePower::from(2 * 1048576));
    let allowance_to_remove = DataCap::from(StoragePower::from(1048576));

    // register verifier1 and verifier2
    add_verifier(&v, verifier1, &verifier_allowance);
    add_verifier(&v, verifier2, &verifier_allowance);

    // register the verified client
    let add_verified_client_params =
        AddVerifierClientParams { address: verified_client, allowance: verifier_allowance.clone() };
    let add_verified_client_params_ser =
        serialize(&add_verified_client_params, "add verifier params").unwrap();
    let mint_params = MintParams { to: verified_client, amount: verifier_allowance.to_tokens() };
    apply_ok(
        &v,
        verifier1,
        *VERIFIED_REGISTRY_ACTOR_ADDR,
        TokenAmount::zero(),
        VerifregMethod::AddVerifiedClient as u64,
        add_verified_client_params,
    );

    ExpectInvocation {
        to: *VERIFIED_REGISTRY_ACTOR_ADDR,
        method: VerifregMethod::AddVerifiedClient as u64,
        params: Some(add_verified_client_params_ser),
        subinvocs: Some(vec![ExpectInvocation {
            to: *DATACAP_TOKEN_ACTOR_ADDR,
            method: DataCapMethod::Mint as u64,
            params: Some(serialize(&mint_params, "mint params").unwrap()),
            subinvocs: None,
            ..Default::default()
        }]),
        ..Default::default()
    }
    .matches(v.take_invocations().last().unwrap());

    // state checks on the 2 verifiers and the client
    let mut v_st = v.get_state::<VerifregState>(*VERIFIED_REGISTRY_ACTOR_ADDR).unwrap();
    let verifiers =
        make_map_with_root_and_bitwidth::<_, DataCap>(&v_st.verifiers, &store, HAMT_BIT_WIDTH)
            .unwrap();

    let verifier1_data_cap = verifiers.get(&verifier1_id_addr.to_bytes()).unwrap().unwrap();
    assert_eq!(DataCap::zero(), *verifier1_data_cap);

    let verifier2_data_cap = verifiers.get(&verifier2_id_addr.to_bytes()).unwrap().unwrap();
    assert_eq!(verifier_allowance, *verifier2_data_cap);

    // let mut verified_clients = make_map_with_root_and_bitwidth::<_, BigIntDe>(
    //     &v_st.verified_clients,
    //     &store,
    //     HAMT_BIT_WIDTH,
    // )
    // .unwrap();
    //
    // let BigIntDe(data_cap) =
    //     verified_clients.get(&verified_client_id_addr.to_bytes()).unwrap().unwrap();
    // assert_eq!(*data_cap, verifier_allowance);

    let dc_st = v.get_state::<DataCapState>(*DATACAP_TOKEN_ACTOR_ADDR).unwrap();
    let balance = dc_st.balance(&store, verified_client_id_addr.id().unwrap()).unwrap();
    assert_eq!(balance, verifier_allowance.to_tokens());

    let mut proposal_ids = make_map_with_root_and_bitwidth::<_, RemoveDataCapProposalID>(
        &v_st.remove_data_cap_proposal_ids,
        &store,
        HAMT_BIT_WIDTH,
    )
    .unwrap();

    assert!(proposal_ids
        .get(&AddrPairKey::new(verifier1_id_addr, verified_client_id_addr).to_bytes())
        .unwrap()
        .is_none());

    assert!(proposal_ids
        .get(&AddrPairKey::new(verifier2_id_addr, verified_client_id_addr).to_bytes())
        .unwrap()
        .is_none());

    // remove half the client's allowance
    let mut verifier1_proposal = RemoveDataCapProposal {
        verified_client: verified_client_id_addr,
        data_cap_amount: allowance_to_remove.clone(),
        removal_proposal_id: RemoveDataCapProposalID(0),
    };

    let mut verifier1_proposal_ser = to_vec(&verifier1_proposal).unwrap();
    let mut verifier1_payload = SIGNATURE_DOMAIN_SEPARATION_REMOVE_DATA_CAP.to_vec();
    verifier1_payload.append(&mut verifier1_proposal_ser);

    let mut verifier2_proposal = RemoveDataCapProposal {
        verified_client: verified_client_id_addr,
        data_cap_amount: allowance_to_remove.clone(),
        removal_proposal_id: RemoveDataCapProposalID(0),
    };

    let mut verifier2_proposal_ser = to_vec(&verifier2_proposal).unwrap();
    let mut verifier2_payload = SIGNATURE_DOMAIN_SEPARATION_REMOVE_DATA_CAP.to_vec();
    verifier2_payload.append(&mut verifier2_proposal_ser);

    let mut remove_datacap_params = RemoveDataCapParams {
        verified_client_to_remove: verified_client_id_addr,
        data_cap_amount_to_remove: allowance_to_remove.clone(),
        verifier_request_1: RemoveDataCapRequest {
            verifier: verifier1_id_addr,
            signature: Signature { sig_type: SignatureType::Secp256k1, bytes: verifier1_payload },
        },
        verifier_request_2: RemoveDataCapRequest {
            verifier: verifier2_id_addr,
            signature: Signature { sig_type: SignatureType::Secp256k1, bytes: verifier2_payload },
        },
    };

    let mut remove_datacap_params_ser =
        serialize(&remove_datacap_params, "add verifier params").unwrap();
    // let destroy_params =
    //     DestroyParams { owner: verified_client, amount: allowance_to_remove.to_tokens() };
    // let destroy_params_ser = serialize(&destroy_params, "destroy params").unwrap();

    let remove_datacap_ret: RemoveDataCapReturn = apply_ok(
        &v,
        TEST_VERIFREG_ROOT_ADDR,
        *VERIFIED_REGISTRY_ACTOR_ADDR,
        TokenAmount::zero(),
        VerifregMethod::RemoveVerifiedClientDataCap as u64,
        remove_datacap_params,
    )
    .deserialize()
    .unwrap();

    ExpectInvocation {
        to: *VERIFIED_REGISTRY_ACTOR_ADDR,
        method: VerifregMethod::RemoveVerifiedClientDataCap as u64,
        params: Some(remove_datacap_params_ser),
        subinvocs: Some(vec![]),
        ..Default::default()
    }
    .matches(v.take_invocations().last().unwrap());

    assert_eq!(verified_client_id_addr, remove_datacap_ret.verified_client);
    assert_eq!(allowance_to_remove, remove_datacap_ret.data_cap_removed);

    // confirm client's allowance has fallen by half
    // verified_clients = make_map_with_root_and_bitwidth::<_, BigIntDe>(
    //     &v_st.verified_clients,
    //     &store,
    //     HAMT_BIT_WIDTH,
    // )
    // .unwrap();
    //
    // let BigIntDe(data_cap) =
    //     verified_clients.get(&verified_client_id_addr.to_bytes()).unwrap().unwrap();
    //
    // assert_eq!(*data_cap, verifier_allowance.sub(allowance_to_remove.clone()));

    let dc_st = v.get_state::<DataCapState>(*DATACAP_TOKEN_ACTOR_ADDR).unwrap();
    let balance = dc_st.balance(&store, verified_client_id_addr.id().unwrap()).unwrap();
    assert_eq!(balance, verifier_allowance.sub(&allowance_to_remove).to_tokens());

    v_st = v.get_state::<VerifregState>(*VERIFIED_REGISTRY_ACTOR_ADDR).unwrap();
    // confirm proposalIds has changed as expected
    proposal_ids =
        make_map_with_root_and_bitwidth(&v_st.remove_data_cap_proposal_ids, &store, HAMT_BIT_WIDTH)
            .unwrap();

    let verifier1_proposal_id: &RemoveDataCapProposalID = proposal_ids
        .get(&AddrPairKey::new(verifier1_id_addr, verified_client_id_addr).to_bytes())
        .unwrap()
        .unwrap();

    assert_eq!(1u64, verifier1_proposal_id.0);

    let verifier2_proposal_id: &RemoveDataCapProposalID = proposal_ids
        .get(&AddrPairKey::new(verifier2_id_addr, verified_client_id_addr).to_bytes())
        .unwrap()
        .unwrap();

    assert_eq!(1u64, verifier2_proposal_id.0);

    // remove the second half of the client's allowance, this causes the client to be deleted

    verifier1_proposal = RemoveDataCapProposal {
        verified_client: verified_client_id_addr,
        data_cap_amount: allowance_to_remove.clone(),
        removal_proposal_id: verifier1_proposal_id.clone(),
    };

    verifier1_proposal_ser = to_vec(&verifier1_proposal).unwrap();
    verifier1_payload = SIGNATURE_DOMAIN_SEPARATION_REMOVE_DATA_CAP.to_vec();
    verifier1_payload.append(&mut verifier1_proposal_ser);

    verifier2_proposal = RemoveDataCapProposal {
        verified_client: verified_client_id_addr,
        data_cap_amount: allowance_to_remove.clone(),
        removal_proposal_id: verifier2_proposal_id.clone(),
    };

    verifier2_proposal_ser = to_vec(&verifier2_proposal).unwrap();
    verifier2_payload = SIGNATURE_DOMAIN_SEPARATION_REMOVE_DATA_CAP.to_vec();
    verifier2_payload.append(&mut verifier2_proposal_ser);

    remove_datacap_params = RemoveDataCapParams {
        verified_client_to_remove: verified_client_id_addr,
        data_cap_amount_to_remove: allowance_to_remove.clone(),
        verifier_request_1: RemoveDataCapRequest {
            verifier: verifier1_id_addr,
            signature: Signature { sig_type: SignatureType::Secp256k1, bytes: verifier1_payload },
        },
        verifier_request_2: RemoveDataCapRequest {
            verifier: verifier2_id_addr,
            signature: Signature { sig_type: SignatureType::Secp256k1, bytes: verifier2_payload },
        },
    };

    remove_datacap_params_ser = serialize(&remove_datacap_params, "add verifier params").unwrap();

    let remove_datacap_ret: RemoveDataCapReturn = apply_ok(
        &v,
        TEST_VERIFREG_ROOT_ADDR,
        *VERIFIED_REGISTRY_ACTOR_ADDR,
        TokenAmount::zero(),
        VerifregMethod::RemoveVerifiedClientDataCap as u64,
        remove_datacap_params,
    )
    .deserialize()
    .unwrap();

    ExpectInvocation {
        to: *VERIFIED_REGISTRY_ACTOR_ADDR,
        method: VerifregMethod::RemoveVerifiedClientDataCap as u64,
        params: Some(remove_datacap_params_ser),
        subinvocs: Some(vec![]),
        ..Default::default()
    }
    .matches(v.take_invocations().last().unwrap());

    assert_eq!(verified_client_id_addr, remove_datacap_ret.verified_client);
    assert_eq!(allowance_to_remove, remove_datacap_ret.data_cap_removed);

    // confirm client has no balance
    // v_st = v.get_state::<VerifregState>(*VERIFIED_REGISTRY_ACTOR_ADDR).unwrap();
    // verified_clients = make_map_with_root_and_bitwidth::<_, BigIntDe>(
    //     &v_st.verified_clients,
    //     &store,
    //     HAMT_BIT_WIDTH,
    // )
    // .unwrap();
    //
    // assert!(verified_clients.get(&verified_client_id_addr.to_bytes()).unwrap().is_none());

    let dc_st = v.get_state::<DataCapState>(*DATACAP_TOKEN_ACTOR_ADDR).unwrap();
    let balance = dc_st.balance(&store, verified_client_id_addr.id().unwrap()).unwrap();
    assert_eq!(balance, TokenAmount::zero());

    // confirm proposalIds has changed as expected
    proposal_ids =
        make_map_with_root_and_bitwidth(&v_st.remove_data_cap_proposal_ids, &store, HAMT_BIT_WIDTH)
            .unwrap();

    let verifier1_proposal_id: &RemoveDataCapProposalID = proposal_ids
        .get(&AddrPairKey::new(verifier1_id_addr, verified_client_id_addr).to_bytes())
        .unwrap()
        .unwrap();

    assert_eq!(2u64, verifier1_proposal_id.0);

    let verifier2_proposal_id: &RemoveDataCapProposalID = proposal_ids
        .get(&AddrPairKey::new(verifier2_id_addr, verified_client_id_addr).to_bytes())
        .unwrap()
        .unwrap();

    assert_eq!(2u64, verifier2_proposal_id.0);
    v.assert_state_invariants();
}
