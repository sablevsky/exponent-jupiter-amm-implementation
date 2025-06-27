use anchor_lang::prelude::*;
use std::collections::HashMap;

/// Account lists for validating CPI calls to the SY program
#[derive(AnchorDeserialize, AnchorSerialize, Default, Clone, Debug)]
pub struct CpiAccounts {
    /// Fetch SY state
    pub get_sy_state: Vec<CpiInterfaceContext>,

    /// Deposit SY into personal account owned by vault
    pub deposit_sy: Vec<CpiInterfaceContext>,

    /// Withdraw SY from personal account owned by vault
    pub withdraw_sy: Vec<CpiInterfaceContext>,

    /// Settle rewards for vault to accounts owned by the vault
    pub claim_emission: Vec<Vec<CpiInterfaceContext>>,

    /// Get personal yield position
    pub get_position_state: Vec<CpiInterfaceContext>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CpiInterfaceContext {
    /// Address-lookup-table index
    pub alt_index: u8,
    pub is_signer: bool,
    pub is_writable: bool,
}

/**
 * Get Vec<CpiInterfaceContext> and return Vec<CpiInterfaceContext> with unique alt_index
 * Prioritizes structs with is_writable=true and/or is_signer=true
 */
pub fn unique_cpi_contexts(contexts: &[CpiInterfaceContext]) -> Vec<CpiInterfaceContext> {
    let mut seen: HashMap<u8, CpiInterfaceContext> = HashMap::new();

    for context in contexts {
        let entry = seen
            .entry(context.alt_index)
            .or_insert_with(|| context.clone());
        entry.is_writable |= context.is_writable;
        entry.is_signer |= context.is_signer;
    }

    return seen.into_values().collect();
}

/**
 * Convert Vec<CpiInterfaceContext> to Vec<AccountMeta> using Vec<Pubkey> from lookup table
 */
pub fn cpi_contexts_to_account_metas(
    contexts: &[CpiInterfaceContext],
    pubkeys: &[Pubkey],
) -> Vec<AccountMeta> {
    contexts
        .iter()
        .filter_map(|ctx| {
            let idx = ctx.alt_index as usize;
            pubkeys.get(idx).map(|pk| {
                if ctx.is_writable {
                    AccountMeta::new(*pk, ctx.is_signer)
                } else {
                    AccountMeta::new_readonly(*pk, ctx.is_signer)
                }
            })
        })
        .collect()
}
