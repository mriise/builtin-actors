// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::Cbor;
use fvm_shared::address::Address;
use fvm_shared::error::ExitCode;
use fvm_shared::HAMT_BIT_WIDTH;

use crate::DataCap;
use fil_actors_runtime::{
    actor_error, make_empty_map, make_map_with_root_and_bitwidth, ActorError, AsActorError,
};

#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct State {
    pub root_key: Address,
    pub token: Address,
    pub verifiers: Cid,
    pub remove_data_cap_proposal_ids: Cid,
}

impl State {
    pub fn new<BS: Blockstore>(
        store: &BS,
        root_key: Address,
        token: Address,
    ) -> Result<State, ActorError> {
        let empty_map = make_empty_map::<_, ()>(store, HAMT_BIT_WIDTH)
            .flush()
            .map_err(|e| actor_error!(illegal_state, "failed to create empty map: {}", e))?;

        Ok(State { root_key, token, verifiers: empty_map, remove_data_cap_proposal_ids: empty_map })
    }

    // Adds a verifier and cap, overwriting any existing cap for that verifier.
    pub fn put_verifier(
        &mut self,
        store: &impl Blockstore,
        verifier: &Address,
        cap: &DataCap,
    ) -> Result<(), ActorError> {
        let mut verifiers =
            make_map_with_root_and_bitwidth::<_, DataCap>(&self.verifiers, store, HAMT_BIT_WIDTH)
                .context_code(ExitCode::USR_ILLEGAL_STATE, "failed to load verifiers")?;
        // .context("failed to load verifiers")?;
        verifiers
            .set(verifier.to_bytes().into(), cap.clone())
            .context_code(ExitCode::USR_ILLEGAL_STATE, "failed to set verifier")?;
        self.verifiers = verifiers
            .flush()
            .context_code(ExitCode::USR_ILLEGAL_STATE, "failed to flush verifiers")?;
        Ok(())
    }

    pub fn remove_verifier(
        &mut self,
        store: &impl Blockstore,
        verifier: &Address,
    ) -> Result<(), ActorError> {
        let mut verifiers =
            make_map_with_root_and_bitwidth::<_, DataCap>(&self.verifiers, store, HAMT_BIT_WIDTH)
                .context_code(ExitCode::USR_ILLEGAL_STATE, "failed to load verifiers")?;

        verifiers
            .delete(&verifier.to_bytes())
            .context_code(ExitCode::USR_ILLEGAL_STATE, "failed to remove verifier")?
            .context_code(ExitCode::USR_ILLEGAL_ARGUMENT, "verifier not found")?;

        self.verifiers = verifiers
            .flush()
            .context_code(ExitCode::USR_ILLEGAL_STATE, "failed to flush verifiers")?;
        Ok(())
    }

    pub fn get_verifier_cap(
        &self,
        store: &impl Blockstore,
        verifier: &Address,
    ) -> Result<Option<DataCap>, ActorError> {
        let verifiers =
            make_map_with_root_and_bitwidth::<_, DataCap>(&self.verifiers, store, HAMT_BIT_WIDTH)
                .context_code(ExitCode::USR_ILLEGAL_STATE, "failed to load verifiers")?;
        let allowance = verifiers
            .get(&verifier.to_bytes())
            .context_code(ExitCode::USR_ILLEGAL_STATE, "failed to get verifier")?;
        Ok(allowance.cloned())
    }
}

impl Cbor for State {}
