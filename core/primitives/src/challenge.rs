use crate::hash::CryptoHash;
use crate::merkle::MerklePath;
use crate::sharding::{EncodedShardChunk, ShardChunk, ShardChunkHeader};
use crate::types::AccountId;
use crate::validator_signer::ValidatorSigner;
use borsh::{BorshDeserialize, BorshSerialize};
use near_crypto::Signature;
use near_schema_checker_lib::ProtocolSchema;
use std::fmt::{Debug, Formatter};

/// Serialized TrieNodeWithSize or state value.
pub type TrieValue = std::sync::Arc<[u8]>;

#[derive(BorshSerialize, BorshDeserialize, Clone, Eq, PartialEq, ProtocolSchema)]
/// TODO (#8984): consider supporting format containing trie values only for
/// state part boundaries and storing state items for state part range.
pub enum PartialState {
    /// State represented by the set of unique trie values (`RawTrieNodeWithSize`s and state values).
    TrieValues(Vec<TrieValue>),
}

impl Default for PartialState {
    fn default() -> Self {
        PartialState::TrieValues(vec![])
    }
}

// When debug-printing, don't dump the entire partial state; that is very unlikely to be useful,
// and wastes a lot of screen space.
impl Debug for PartialState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PartialState::TrieValues(values) => {
                f.write_str(&format!("{} trie values", values.len()))
            }
        }
    }
}

impl PartialState {
    pub fn len(&self) -> usize {
        let Self::TrieValues(values) = self;
        values.len()
    }
}

/// Double signed block.
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Debug, ProtocolSchema)]
pub struct BlockDoubleSign {
    pub left_block_header: Vec<u8>,
    pub right_block_header: Vec<u8>,
}

impl std::fmt::Display for BlockDoubleSign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

/// Invalid chunk (body of the chunk doesn't match proofs or invalid encoding).
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Debug, ProtocolSchema)]
pub struct ChunkProofs {
    /// Encoded block header that contains invalid chunk.
    pub block_header: Vec<u8>,
    /// Merkle proof of inclusion of this chunk.
    pub merkle_proof: MerklePath,
    /// Invalid chunk in an encoded form or in a decoded form.
    pub chunk: Box<MaybeEncodedShardChunk>,
}

/// Either `EncodedShardChunk` or `ShardChunk`. Used for `ChunkProofs`.
/// `Decoded` is used to avoid re-encoding an already decoded chunk to construct a challenge.
/// `Encoded` is still needed in case a challenge challenges an invalid encoded chunk that can't be
/// decoded.
#[allow(clippy::large_enum_variant)] // both variants are large
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Debug, ProtocolSchema)]
pub enum MaybeEncodedShardChunk {
    Encoded(EncodedShardChunk),
    Decoded(ShardChunk),
}

/// Doesn't match post-{state root, outgoing receipts, gas used, etc} results after applying previous chunk.
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Debug, ProtocolSchema)]
pub struct ChunkState {
    /// Encoded prev block header.
    pub prev_block_header: Vec<u8>,
    /// Encoded block header that contains invalid chunnk.
    pub block_header: Vec<u8>,
    /// Merkle proof in inclusion of prev chunk.
    pub prev_merkle_proof: MerklePath,
    /// Previous chunk that contains transactions.
    pub prev_chunk: ShardChunk,
    /// Merkle proof of inclusion of this chunk.
    pub merkle_proof: MerklePath,
    /// Invalid chunk header.
    pub chunk_header: ShardChunkHeader,
    /// Partial state that was affected by transactions of given chunk.
    pub partial_state: PartialState,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Debug, ProtocolSchema)]
// TODO(#1313): Use Box
#[allow(clippy::large_enum_variant)]
pub enum ChallengeBody {
    BlockDoubleSign(BlockDoubleSign),
    ChunkProofs(ChunkProofs),
    ChunkState(ChunkState),
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Debug, ProtocolSchema)]
#[borsh(init=init)]
pub struct Challenge {
    pub body: ChallengeBody,
    pub account_id: AccountId,
    pub signature: Signature,

    #[borsh(skip)]
    pub hash: CryptoHash,
}

impl Challenge {
    pub fn init(&mut self) {
        self.hash = CryptoHash::hash_borsh(&self.body);
    }

    pub fn produce(body: ChallengeBody, signer: &ValidatorSigner) -> Self {
        let hash = CryptoHash::hash_borsh(&body);
        let signature = signer.sign_bytes(hash.as_ref());
        Self { body, account_id: signer.validator_id().clone(), signature, hash }
    }
}

pub type Challenges = Vec<Challenge>;

#[derive(
    BorshSerialize,
    BorshDeserialize,
    PartialEq,
    Eq,
    Clone,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    ProtocolSchema,
)]
pub struct SlashedValidator {
    pub account_id: AccountId,
    pub is_double_sign: bool,
}

impl SlashedValidator {
    pub fn new(account_id: AccountId, is_double_sign: bool) -> Self {
        SlashedValidator { account_id, is_double_sign }
    }
}

/// Result of checking challenge, contains which accounts to slash.
/// If challenge is invalid this is sender, otherwise author of chunk (and possibly other participants that signed invalid blocks).
pub type ChallengesResult = Vec<SlashedValidator>;
