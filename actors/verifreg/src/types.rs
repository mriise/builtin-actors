// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cmp::Ordering;
use std::fmt::Display;
use std::ops::{Add, Div, Sub};

use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::Cbor;
use fvm_shared::address::Address;
use fvm_shared::bigint::{bigint_ser, BigInt};
use fvm_shared::crypto::signature::Signature;
use fvm_shared::econ::TokenAmount;
use fvm_shared::sector::StoragePower;
use num_traits::{Signed, Zero};
use serde::{Deserialize, Serialize, Serializer};

use crate::ext::datacap::TOKEN_PRECISION;

#[derive(Clone, Debug, PartialEq, Eq, Serialize_tuple, Deserialize_tuple)]
pub struct VerifierParams {
    pub address: Address,
    pub allowance: DataCap,
}

impl Cbor for VerifierParams {}

pub type AddVerifierParams = VerifierParams;

pub type AddVerifierClientParams = VerifierParams;

/// DataCap is an integer number of bytes.
/// This is a new-type in order to prevent accidental conversion from TokenAmount,
/// which has an implicit 18-dp precision.
/// Transparently convertible from StoragePower, but must be explicitly converted to/from
/// TokenAmount (which multiplies by the token precision).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataCap(BigInt);

impl DataCap {
    pub fn as_power(&self) -> &StoragePower {
        &self.0
    }

    pub fn zero() -> Self {
        DataCap(StoragePower::zero())
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn is_positive(&self) -> bool {
        self.0.is_positive()
    }

    pub fn is_negative(&self) -> bool {
        self.0.is_negative()
    }

    /// Converts a standard token quantity into a data cap quantity (dividing by token precision).
    /// Truncates any remainder.
    pub fn from_tokens(t: &TokenAmount) -> Self {
        DataCap(t.div(TOKEN_PRECISION))
    }

    /// Converts this data cap to a token quantity (multiplying by token precision).
    pub fn to_tokens(&self) -> TokenAmount {
        &self.0 * TOKEN_PRECISION
    }
}

impl PartialOrd for DataCap {
    #[inline]
    fn partial_cmp(&self, other: &DataCap) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<'a> Sub<&'a DataCap> for DataCap {
    type Output = DataCap;
    fn sub(self, other: &DataCap) -> DataCap {
        DataCap(self.0 - &other.0)
    }
}

impl<'a> Add<&'a DataCap> for DataCap {
    type Output = DataCap;
    fn add(self, other: &DataCap) -> DataCap {
        DataCap(self.0 + &other.0)
    }
}

impl<'a, 'b> Add<&'a DataCap> for &'b DataCap {
    type Output = DataCap;
    fn add(self, other: &DataCap) -> DataCap {
        DataCap(&self.0 + &other.0)
    }
}

impl From<i32> for DataCap {
    fn from(i: i32) -> Self {
        DataCap(BigInt::from(i))
    }
}

impl From<BigInt> for DataCap {
    fn from(i: BigInt) -> Self {
        DataCap(i)
    }
}

impl From<&BigInt> for DataCap {
    fn from(i: &BigInt) -> Self {
        DataCap(i.clone())
    }
}

impl Display for DataCap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl Serialize for DataCap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        bigint_ser::serialize(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for DataCap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        bigint_ser::deserialize(deserializer).map(DataCap)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize_tuple, Deserialize_tuple)]
pub struct BytesParams {
    /// Address of verified client.
    pub address: Address,
    /// Number of bytes to use.
    pub deal_size: DataCap,
}

pub type UseBytesParams = BytesParams;
pub type RestoreBytesParams = BytesParams;

pub const SIGNATURE_DOMAIN_SEPARATION_REMOVE_DATA_CAP: &[u8] = b"fil_removedatacap:";

impl Cbor for RemoveDataCapParams {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize_tuple, Deserialize_tuple)]
pub struct RemoveDataCapParams {
    pub verified_client_to_remove: Address,
    pub data_cap_amount_to_remove: DataCap,
    pub verifier_request_1: RemoveDataCapRequest,
    pub verifier_request_2: RemoveDataCapRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize_tuple, Deserialize_tuple)]
pub struct RemoveDataCapRequest {
    pub verifier: Address,
    pub signature: Signature,
}

impl Cbor for RemoveDataCapReturn {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize_tuple, Deserialize_tuple)]
pub struct RemoveDataCapReturn {
    pub verified_client: Address,
    pub data_cap_removed: DataCap,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(transparent)]
pub struct RemoveDataCapProposalID(pub u64);

#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
pub struct RemoveDataCapProposal {
    pub verified_client: Address,
    pub data_cap_amount: DataCap,
    pub removal_proposal_id: RemoveDataCapProposalID,
}

pub struct AddrPairKey {
    pub first: Address,
    pub second: Address,
}

impl AddrPairKey {
    pub fn new(first: Address, second: Address) -> Self {
        AddrPairKey { first, second }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut first = self.first.to_bytes();
        let mut second = self.second.to_bytes();
        first.append(&mut second);
        first
    }
}
