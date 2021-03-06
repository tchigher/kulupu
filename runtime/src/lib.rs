// Copyright 2019-2020 Wei Tang.
// This file is part of Kulupu.

// Kulupu is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Kulupu is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Kulupu.  If not, see <http://www.gnu.org/licenses/>.

//! The Kulupu runtime. This can be compiled with `#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit="256"]

mod fee;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use sp_std::{collections::btree_map::BTreeMap, prelude::*};
use codec::{Encode, Decode};
use sp_core::{OpaqueMetadata, u32_trait::{_1, _2, _4, _5}};
use sp_runtime::{
	ApplyExtrinsicResult, Percent, ModuleId, generic, create_runtime_str, MultiSignature,
	RuntimeDebug, Perquintill, transaction_validity::{TransactionValidity, TransactionSource},
	FixedPointNumber,
};
use sp_runtime::traits::{
	BlakeTwo256, Block as BlockT, StaticLookup, Saturating,
	Verify, IdentifyAccount, Convert, ConvertInto,
};
use sp_api::impl_runtime_apis;
use sp_version::RuntimeVersion;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use kulupu_primitives::{DOLLARS, CENTS, MILLICENTS, MICROCENTS, HOURS, DAYS, BLOCK_TIME, deposit};
use transaction_payment::{TargetedFeeAdjustment, Multiplier};
use system::EnsureRoot;
use crate::fee::WeightToFee;

// A few exports that help ease life for downstream crates.
pub use sp_runtime::{Permill, Perbill};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use frame_support::{
	StorageValue, construct_runtime, parameter_types,
	traits::{Currency, Randomness, LockIdentifier, OnUnbalanced, InstanceFilter},
	weights::{
		Weight, RuntimeDbWeight,
		constants::{
			WEIGHT_PER_SECOND, BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight
		},
	},
};
pub use timestamp::Call as TimestampCall;
pub use balances::Call as BalancesCall;

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts.
pub type AccountIndex = u32;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// Digest item type.
pub type DigestItem = generic::DigestItem<Hash>;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core datastructures.
pub mod opaque {
	use super::*;

	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;
}

/// This runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("kulupu"),
	impl_name: create_runtime_str!("kulupu"),
	authoring_version: 5,
	spec_version: 8,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 5,
};

/// The version infromation used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

const AVERAGE_ON_INITIALIZE_WEIGHT: Perbill = Perbill::from_percent(10);
parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
	pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
	pub const MaximumBlockWeight: Weight = 2 * WEIGHT_PER_SECOND;
	pub MaximumExtrinsicWeight: Weight = AvailableBlockRatio::get()
		.saturating_sub(AVERAGE_ON_INITIALIZE_WEIGHT) * MaximumBlockWeight::get();
	pub const MaximumBlockLength: u32 = 5 * 1024 * 1024;
	pub const Version: RuntimeVersion = VERSION;
	pub const DbWeight: RuntimeDbWeight = frame_support::weights::constants::RocksDbWeight::get();
}

impl system::Trait for Runtime {
	/// Filter for base call.
	type BaseCallFilter = ();
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The aggregated dispatch type that is available for extrinsics.
	type Call = Call;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = Indices;
	/// The index type for storing how many extrinsics an account has signed.
	type Index = Index;
	/// The index type for blocks.
	type BlockNumber = BlockNumber;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The header type.
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// The ubiquitous event type.
	type Event = Event;
	/// The ubiquitous origin type.
	type Origin = Origin;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// Maximum weight of each block.
	type MaximumBlockWeight = MaximumBlockWeight;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = DbWeight;
	/// The weight of the overhead invoked on the block import process, independent of the
	/// extrinsics included in that block.
	type BlockExecutionWeight = BlockExecutionWeight;
	/// The base weight of any extrinsic processed by the runtime, independent of the
	/// logic of that extrinsic. (Signature verification, nonce increment, fee, etc...)
	type ExtrinsicBaseWeight = ExtrinsicBaseWeight;
	/// Maximum size of all encoded transactions (in bytes) that are allowed in one block.
	type MaximumBlockLength = MaximumBlockLength;
	/// Maximum extrinsic weight.
	type MaximumExtrinsicWeight = MaximumExtrinsicWeight;
	/// Portion of the block weight that is available to all normal transactions.
	type AvailableBlockRatio = AvailableBlockRatio;
	/// Version of the runtime.
	type Version = Version;
	/// Converts a module to the index of the module in `construct_runtime!`.
	///
	/// This type is being generated by `construct_runtime!`.
	type ModuleToIndex = ModuleToIndex;
	/// What to do if a new account is created.
	type OnNewAccount = ();
	/// What to do if an account is fully reaped from the system.
	type OnKilledAccount = ();
	/// The data to be stored in an account.
	type AccountData = balances::AccountData<Balance>;
	/// System weight info.
	type SystemWeightInfo = ();
}

impl scheduler::Trait for Runtime {
	type Event = Event;
	type Origin = Origin;
	type Call = Call;
	type MaximumWeight = MaximumBlockWeight;
	type PalletsOrigin = OriginCaller;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
}

parameter_types! {
	// One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
	pub const DepositBase: Balance = deposit(1, 88);
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = deposit(0, 32);
	pub const MaxSignatories: u16 = 100;
}

impl multisig::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type WeightInfo = ();
}

impl utility::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type WeightInfo = ();
}

parameter_types! {
	pub const IndexDeposit: Balance = 1 * DOLLARS;
}

impl indices::Trait for Runtime {
	/// The type for recording indexing into the account enumeration.
	type AccountIndex = AccountIndex;
	/// Index deposit.
	type Deposit = IndexDeposit;
	/// Currency of the indices.
	type Currency = Balances;
	/// The ubiquitous event type.
	type Event = Event;
	/// Weight info for indices.
	type WeightInfo = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = 1000;
}

impl timestamp::Trait for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Difficulty;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 10 * MICROCENTS;
}

impl balances::Trait for Runtime {
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
}

type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;

pub struct DealWithFees;
impl OnUnbalanced<NegativeImbalance> for DealWithFees {
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item=NegativeImbalance>) {
		if let Some(fees) = fees_then_tips.next() {
			// Burn base fees.
			drop(fees);
			if let Some(tips) = fees_then_tips.next() {
				// Pay tips to miners.
				Author::on_unbalanced(tips);
			}
		}
	}
}

parameter_types! {
	pub const TransactionByteFee: Balance = 10 * MILLICENTS;
	/// The portion of the `AvailableBlockRatio` that we adjust the fees with. Blocks filled less
	/// than this will decrease the weight and more will increase.
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	/// The adjustment variable of the runtime. Higher values will cause `TargetBlockFullness` to
	/// change the fees more rapidly.
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(3, 100_000);
	/// Minimum amount of the multiplier. This value cannot be too low. A test case should ensure
	/// that combined with `AdjustmentVariable`, we can recover from the minimum.
	/// See `multiplier_can_grow_from_zero`.
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
}

impl transaction_payment::Trait for Runtime {
	type Currency = balances::Module<Runtime>;
	type OnTransactionPayment = DealWithFees;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = WeightToFee;
	type FeeMultiplierUpdate = TargetedFeeAdjustment<Self, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier>;
}

parameter_types! {
	pub const TargetBlockTime: u64 = BLOCK_TIME;
}

impl difficulty::Trait for Runtime {
	type TargetBlockTime = TargetBlockTime;
}

impl eras::Trait for Runtime { }

pub struct GenerateRewardLocks;

impl rewards::GenerateRewardLocks<Runtime> for GenerateRewardLocks {
	fn generate_reward_locks(
		current_block: BlockNumber,
		total_reward: Balance,
	) -> BTreeMap<BlockNumber, Balance> {
		let mut locks = BTreeMap::new();
		let locked_reward = total_reward.saturating_sub(1 * DOLLARS);

		if locked_reward > 0 {
			const TOTAL_LOCK_PERIOD: BlockNumber = 100 * DAYS;
			const DIVIDE: BlockNumber = 10;

			for i in 0..DIVIDE {
				let one_locked_reward = locked_reward / DIVIDE as u128;

				let estimate_block_number = current_block + (i + 1) * (TOTAL_LOCK_PERIOD / DIVIDE);
				let actual_block_number = estimate_block_number / DAYS * DAYS;

				locks.insert(actual_block_number, one_locked_reward);
			}
		}

		locks
	}
}

parameter_types! {
	pub DonationDestination: AccountId = Treasury::account_id();
}

impl rewards::Trait for Runtime {
	type Event = Event;
	type Currency = Balances;
	type DonationDestination = DonationDestination;
	type GenerateRewardLocks = GenerateRewardLocks;
}

pub struct Author;
impl OnUnbalanced<NegativeImbalance> for Author {
	fn on_nonzero_unbalanced(amount: NegativeImbalance) {
		if let Some(author) = Rewards::author() {
			Balances::resolve_creating(&author, amount);
		} else {
			drop(amount);
		}
	}
}

parameter_types! {
	pub const LaunchPeriod: BlockNumber = 7 * DAYS;
	pub const VotingPeriod: BlockNumber = 7 * DAYS;
	pub const FastTrackVotingPeriod: BlockNumber = 1 * DAYS;
	pub const MinimumDeposit: Balance = 100 * DOLLARS;
	pub const EnactmentPeriod: BlockNumber = 8 * DAYS;
	pub const CooloffPeriod: BlockNumber = 7 * DAYS;
	// One cent: $10,000 / MB
	pub const PreimageByteDeposit: Balance = 10 * MILLICENTS;
	pub const InstantAllowed: bool = false;
	pub const MaxVotes: u32 = 100;
}

impl democracy::Trait for Runtime {
	type Proposal = Call;
	type Event = Event;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type MinimumDeposit = MinimumDeposit;
	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>;
	/// A super-majority can have the next scheduled referendum be a straight
	/// majority-carries vote.
	type ExternalMajorityOrigin = collective::EnsureProportionAtLeast<_4, _5, AccountId, CouncilCollective>;
	/// A unanimous council can have the next scheduled referendum be a straight
	/// default-carries (NTB) vote.
	type ExternalDefaultOrigin = collective::EnsureProportionAtLeast<_1, _1, AccountId, CouncilCollective>;
	/// Full of the technical committee can have an
	/// ExternalMajority/ExternalDefault vote be tabled immediately and with a
	/// shorter voting/enactment period.
	type FastTrackOrigin = collective::EnsureProportionAtLeast<_1, _1, AccountId, TechnicalCollective>;
	type InstantOrigin = system::EnsureNever<AccountId>;
	type InstantAllowed = InstantAllowed;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	/// To cancel a proposal which has been passed, all of the council must
	/// agree to it.
	type CancellationOrigin = collective::EnsureProportionAtLeast<_1, _1, AccountId, CouncilCollective>;
	type OperationalPreimageOrigin = collective::EnsureMember<AccountId, CouncilCollective>;
	/// Any single technical committee member may veto a coming council
	/// proposal, however they can only do it once and it lasts only for the
	/// cooloff period.
	type VetoOrigin = collective::EnsureMember<AccountId, TechnicalCollective>;
	type CooloffPeriod = CooloffPeriod;
	type PreimageByteDeposit = PreimageByteDeposit;
	type Slash = Treasury;
	type Scheduler = Scheduler;
	type MaxVotes = MaxVotes;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = ();
}

parameter_types! {
	pub const CouncilMotionDuration: BlockNumber = 3 * DAYS;
	pub const CouncilMaxProposals: u32 = 100;
}

type CouncilCollective = collective::Instance1;
impl collective::Trait<CouncilCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = CouncilMotionDuration;
	type MaxProposals = CouncilMaxProposals;
	type WeightInfo = ();
}

/// Converter for currencies to votes.
pub struct CurrencyToVoteHandler<R>(sp_std::marker::PhantomData<R>);

impl<R> CurrencyToVoteHandler<R>
where
	R: balances::Trait,
	R::Balance: Into<u128>,
{
	fn factor() -> u128 {
		let issuance: u128 = <balances::Module<R>>::total_issuance().into();
		(issuance / u64::max_value() as u128).max(1)
	}
}

impl<R> Convert<u128, u64> for CurrencyToVoteHandler<R>
where
	R: balances::Trait,
	R::Balance: Into<u128>,
{
	fn convert(x: u128) -> u64 { (x / Self::factor()) as u64 }
}

impl<R> Convert<u128, u128> for CurrencyToVoteHandler<R>
where
	R: balances::Trait,
	R::Balance: Into<u128>,
{
	fn convert(x: u128) -> u128 { x * Self::factor() }
}

parameter_types! {
	pub const CandidacyBond: Balance = 1 * DOLLARS;
	pub const VotingBond: Balance = 5 * CENTS;
	/// Daily council elections.
	pub const TermDuration: BlockNumber = 24 * HOURS;
	pub const DesiredMembers: u32 = 17;
	pub const DesiredRunnersUp: u32 = 30;
	pub const ElectionsPhragmenModuleId: LockIdentifier = *b"phrelect";
}

impl elections_phragmen::Trait for Runtime {
	type Event = Event;
	type Currency = Balances;
	type ChangeMembers = Council;
	type InitializeMembers = Council;
	type CurrencyToVote = CurrencyToVoteHandler<Self>;
	type CandidacyBond = CandidacyBond;
	type VotingBond = VotingBond;
	type LoserCandidate = Treasury;
	type BadReport = Treasury;
	type KickedMember = Treasury;
	type DesiredMembers = DesiredMembers;
	type DesiredRunnersUp = DesiredRunnersUp;
	type TermDuration = TermDuration;
	type ModuleId = ElectionsPhragmenModuleId;
	type WeightInfo = ();
}

parameter_types! {
	pub const TechnicalMotionDuration: BlockNumber = 3 * DAYS;
	pub const TechnicalMaxProposals: u32 = 100;
}

type TechnicalCollective = collective::Instance2;
impl collective::Trait<TechnicalCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = TechnicalMotionDuration;
	type MaxProposals = TechnicalMaxProposals;
	type WeightInfo = ();
}

impl membership::Trait<membership::Instance1> for Runtime {
	type Event = Event;
	type AddOrigin = system::EnsureRoot<AccountId>;
	type RemoveOrigin = system::EnsureRoot<AccountId>;
	type SwapOrigin = system::EnsureRoot<AccountId>;
	type ResetOrigin = system::EnsureRoot<AccountId>;
	type PrimeOrigin = system::EnsureRoot<AccountId>;
	type MembershipInitialized = TechnicalCommittee;
	type MembershipChanged = TechnicalCommittee;
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 20 * DOLLARS;
	pub const SpendPeriod: BlockNumber = 6 * DAYS;
	pub const Burn: Permill = Permill::from_percent(1);
	pub const TreasuryModuleId: ModuleId = ModuleId(*b"py/trsry");

	pub const TipCountdown: BlockNumber = 1 * DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(20);
	pub const TipReportDepositBase: Balance = 1 * DOLLARS;
	pub const TipReportDepositPerByte: Balance = 10 * MILLICENTS;
}

impl treasury::Trait for Runtime {
	type Currency = Balances;
	type ApproveOrigin = collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>;
	type RejectOrigin = collective::EnsureProportionMoreThan<_1, _5, AccountId, CouncilCollective>;
	type Tippers = ElectionsPhragmen;
	type TipCountdown = TipCountdown;
	type TipFindersFee = TipFindersFee;
	type TipReportDepositBase = TipReportDepositBase;
	type TipReportDepositPerByte = TipReportDepositPerByte;
	type Event = Event;
	type ProposalRejection = Treasury;
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type BurnDestination = ();
	type ModuleId = TreasuryModuleId;
	type WeightInfo = ();
}

parameter_types! {
	// Minimum 100 bytes/KSM deposited (1 CENT/byte)
	pub const BasicDeposit: Balance = 10 * DOLLARS;       // 258 bytes on-chain
	pub const FieldDeposit: Balance = 250 * CENTS;        // 66 bytes on-chain
	pub const SubAccountDeposit: Balance = 2 * DOLLARS;   // 53 bytes on-chain
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxAdditionalFields: u32 = 100;
	pub const MaxRegistrars: u32 = 20;
}

impl identity::Trait for Runtime {
	type Event = Event;
	type Currency = Balances;
	type Slashed = Treasury;
	type BasicDeposit = BasicDeposit;
	type FieldDeposit = FieldDeposit;
	type SubAccountDeposit = SubAccountDeposit;
	type MaxSubAccounts = MaxSubAccounts;
	type MaxAdditionalFields = MaxAdditionalFields;
	type MaxRegistrars = MaxRegistrars;
	type RegistrarOrigin = system::EnsureRoot<AccountId>;
	type ForceOrigin = system::EnsureNever<AccountId>;
	type WeightInfo = ();
}

parameter_types! {
	// One storage item; key size 32, value size 8; .
	pub const ProxyDepositBase: Balance = deposit(1, 8);
	// Additional storage item size of 33 bytes.
	pub const ProxyDepositFactor: Balance = deposit(0, 33);
	pub const MaxProxies: u16 = 32;
}

/// The type used to represent the kinds of proxying allowed.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug)]
pub enum ProxyType {
	Any,
	NonTransfer,
	Governance,
	IdentityJudgement,
}
impl Default for ProxyType { fn default() -> Self { Self::Any } }
impl InstanceFilter<Call> for ProxyType {
	fn filter(&self, c: &Call) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::NonTransfer => matches!(c,
				Call::System(..) |
				Call::Timestamp(..) |
				Call::Indices(indices::Call::claim(..)) |
				Call::Indices(indices::Call::free(..)) |
				Call::Indices(indices::Call::freeze(..)) |
				// Specifically omitting Indices `transfer`, `force_transfer`
				// Specifically omitting the entire Balances pallet
				Call::Democracy(..) |
				Call::Council(..) |
				Call::TechnicalCommittee(..) |
				Call::ElectionsPhragmen(..) |
				Call::TechnicalMembership(..) |
				Call::Treasury(..) |
				Call::Utility(..) |
				Call::Identity(..) |
				Call::Vesting(vesting::Call::vest(..)) |
				Call::Vesting(vesting::Call::vest_other(..)) |
				// Specifically omitting Vesting `vested_transfer`, and `force_vested_transfer`
				Call::Scheduler(..) |
				Call::Proxy(..) |
				Call::Multisig(..)
			),
			ProxyType::Governance => matches!(c,
				Call::Democracy(..) | Call::Council(..) | Call::TechnicalCommittee(..)
					| Call::ElectionsPhragmen(..) | Call::Treasury(..) | Call::Utility(..)
			),
			ProxyType::IdentityJudgement => matches!(c,
				Call::Identity(identity::Call::provide_judgement(..))
				| Call::Utility(utility::Call::batch(..))
			)
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		match (self, o) {
			(x, y) if x == y => true,
			(ProxyType::Any, _) => true,
			(_, ProxyType::Any) => false,
			(ProxyType::NonTransfer, _) => true,
			_ => false,
		}
	}
}

impl proxy::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = MaxProxies;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinVestedTransfer: Balance = 10 * DOLLARS;
}

impl vesting::Trait for Runtime {
	type Event = Event;
	type Currency = Balances;
	type BlockNumberToBalance = ConvertInto;
	type MinVestedTransfer = MinVestedTransfer;
	type WeightInfo = ();
}

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		// Basic stuff.
		System: system::{Module, Call, Storage, Config, Event<T>},
		RandomnessCollectiveFlip: randomness_collective_flip::{Module, Call, Storage},
		Timestamp: timestamp::{Module, Call, Storage, Inherent, Config},
		Indices: indices::{Module, Call, Storage, Config<T>, Event<T>},
		Balances: balances::{Module, Call, Storage, Config<T>, Event<T>},
		TransactionPayment: transaction_payment::{Module, Storage},

		// PoW consensus and era support.
		Difficulty: difficulty::{Module, Call, Storage, Config},
		Eras: eras::{Module, Call, Storage, Config<T>},
		Rewards: rewards::{Module, Call, Inherent, Storage, Event<T>, Config<T>},

		// Governance.
		Democracy: democracy::{Module, Call, Storage, Config, Event<T>},
		Council: collective::<Instance1>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		TechnicalCommittee: collective::<Instance2>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		ElectionsPhragmen: elections_phragmen::{Module, Call, Storage, Event<T>, Config<T>},
		TechnicalMembership: membership::<Instance1>::{Module, Call, Storage, Event<T>, Config<T>},
		Treasury: treasury::{Module, Call, Storage, Event<T>, Config},

		// Identity.
		Identity: identity::{Module, Call, Storage, Event<T>},

		// Utility module.
		Utility: utility::{Module, Call, Event},
		Scheduler: scheduler::{Module, Call, Storage, Event<T>},
		Multisig: multisig::{Module, Call, Storage, Event<T>},
		Proxy: proxy::{Module, Call, Storage, Event<T>},
		Vesting: vesting::{Module, Call, Storage, Event<T>, Config<T>},
	}
);

/// The address format for describing accounts.
pub type Address = <Indices as StaticLookup>::Source;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	system::CheckSpecVersion<Runtime>,
	system::CheckTxVersion<Runtime>,
	system::CheckGenesis<Runtime>,
	system::CheckEra<Runtime>,
	system::CheckNonce<Runtime>,
	system::CheckWeight<Runtime>,
	transaction_payment::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<Runtime, Block, system::ChainContext<Runtime>, Runtime, AllModules>;

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			Runtime::metadata().into()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}

		fn random_seed() -> <Block as BlockT>::Hash {
			RandomnessCollectiveFlip::random_seed()
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(_seed: Option<Vec<u8>>) -> Vec<u8> {
			Default::default()
		}

		fn decode_session_keys(
			_encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
			None
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
	}

	impl sp_consensus_pow::TimestampApi<Block, u64> for Runtime {
		fn timestamp() -> u64 {
			timestamp::Module::<Runtime>::get()
		}
	}

	impl sp_consensus_pow::DifficultyApi<Block, kulupu_primitives::Difficulty> for Runtime {
		fn difficulty() -> kulupu_primitives::Difficulty {
			difficulty::Module::<Runtime>::difficulty()
		}
	}

	impl kulupu_primitives::AlgorithmApi<Block> for Runtime {
		fn identifier() -> [u8; 8] {
			kulupu_primitives::ALGORITHM_IDENTIFIER_V2
		}
	}
}
