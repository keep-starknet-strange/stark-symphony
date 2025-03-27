use elements::bitcoin::TapSighashType;
use elements::secp256k1_zkp as secp256k1;
use elements::{
    confidential::{Asset, Nonce, Value},
    hashes::Hash,
    pset::PartiallySignedTransaction,
    sighash::{Prevouts, SighashCache},
    Address, AssetId, AssetIssuance, BlockHash, LockTime, OutPoint, SchnorrSighashType, Script,
    Sequence, Transaction, TxIn, TxInWitness, TxOut, TxOutWitness,
};
use simfony::{CompiledProgram, WitnessValues};

use crate::keys::sign_taproot_keypath;
use crate::script::{create_script, simplicity_leaf_version, taproot_spending_info};

/// Spend a transaction output using P2TR script path
pub fn spend_script_path(
    outpoint: OutPoint,
    utxo: TxOut,
    address: Address,
    key_pair: secp256k1::Keypair,
    program: CompiledProgram,
    witness_values: WitnessValues,
) -> anyhow::Result<Transaction> {
    let value = utxo
        .value
        .explicit()
        .ok_or(anyhow::anyhow!("UTXO value is not explicit"))?;
    let tx = create_transaction(outpoint, address, value, 360);

    let script = create_script(&program)?;
    let (x_only_public_key, _) = key_pair.x_only_public_key();
    let spend_info = taproot_spending_info(script.clone(), x_only_public_key)?;

    let control_block = spend_info
        .control_block(&(script.clone(), simplicity_leaf_version()))
        .unwrap();

    let satisfied_program = program
        .satisfy(witness_values)
        .map_err(|e| anyhow::anyhow!("Failed to satisfy program: {}", e))?;
    let (program_bytes, witness_bytes) = satisfied_program.redeem().encode_to_vec();

    let final_script_witness = vec![
        witness_bytes,
        program_bytes,
        script.into_bytes(),
        control_block.serialize(),
    ];
    // (control[0] & TAPROOT_LEAF_MASK) == TAPROOT_LEAF_TAPSIMPLICITY)
    assert_eq!(final_script_witness[3][0] & 0xfe, 0xbe);

    Ok(finalize_transaction(tx, final_script_witness))
}

/// Spend a transaction output using P2TR key path
pub fn spend_key_path(
    outpoint: OutPoint,
    utxo: TxOut,
    address: Address,
    key_pair: secp256k1::Keypair,
    program: CompiledProgram,
) -> anyhow::Result<Transaction> {
    let value = utxo.value.explicit().unwrap();
    let tx = create_transaction(outpoint, address, value, 150);

    let script = create_script(&program)?;
    let (x_only_public_key, _) = key_pair.x_only_public_key();
    let spend_info = taproot_spending_info(script, x_only_public_key)?;

    let mut sighash_cache = SighashCache::new(&tx);
    let sighash_all = sighash_cache
        .taproot_key_spend_signature_hash(
            outpoint.vout as usize,
            &Prevouts::All(&[utxo]),
            SchnorrSighashType::All,
            liquid_testnet_genesis_hash(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to compute sighash: {}", e))?;

    let signature = sign_taproot_keypath(&sighash_all.to_byte_array(), key_pair, spend_info)?;
    let mut sig = signature.serialize().to_vec();
    sig.push(TapSighashType::All as u8);
    let final_script_witness = vec![sig];

    Ok(finalize_transaction(tx, final_script_witness))
}

/// Create a partially filled transaction with a single input and two outputs:
/// - One is P2TR, locked by our program
/// - The other is a fee output
/// Both outputs are not confidential
fn create_transaction(outpoint: OutPoint, address: Address, value: u64, fee: u64) -> Transaction {
    Transaction {
        version: 2,
        lock_time: LockTime::ZERO.into(),
        input: vec![TxIn {
            previous_output: outpoint,
            is_pegin: false,
            script_sig: Script::new(),
            sequence: Sequence::MAX,
            asset_issuance: AssetIssuance::null(),
            witness: TxInWitness::empty(),
        }],
        output: vec![
            TxOut {
                value: Value::Explicit(value - fee),
                script_pubkey: address.script_pubkey(),
                asset: Asset::Explicit(tlbtc_asset_id()),
                nonce: Nonce::Null,
                witness: TxOutWitness::default(),
            },
            TxOut::new_fee(fee, tlbtc_asset_id()),
        ],
    }
}

/// Add a final input witness to a partially signed transaction
fn finalize_transaction(tx: Transaction, final_script_witness: Vec<Vec<u8>>) -> Transaction {
    let mut partial_tx = PartiallySignedTransaction::from_tx(tx);
    partial_tx.inputs_mut()[0].final_script_witness = Some(final_script_witness);
    partial_tx.extract_tx().unwrap()
}

pub fn tlbtc_asset_id() -> AssetId {
    // NOTE: little endian
    AssetId::from_slice(
        &hex::decode("499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c14").unwrap(),
    )
    .unwrap()
}

pub fn liquid_testnet_genesis_hash() -> BlockHash {
    // NOTE: little endian
    BlockHash::from_slice(
        &hex::decode("c1b16ae24f2423aea2ea34552292793b5b5e82999a1eed81d56aee528eda71a7").unwrap(),
    )
    .unwrap()
}
