// use crate::asset::{Asset, AssetInfo, ClassicmoonInfo};
use crate::asset::{Asset, ClassicmoonInfo};
use crate::classicmoon::{
    QueryMsg as ClassicmoonQueryMsg, ReverseSimulationResponse, SimulationResponse,
};

use classic_bindings::TerraQuery;
use cosmwasm_std::{
    to_binary, Addr, AllBalanceResponse, BalanceResponse, BankQuery, Coin, QuerierWrapper,
    QueryRequest, StdResult, Uint128, WasmQuery,
};

use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20QueryMsg, TokenInfoResponse};
use cw721::{Cw721QueryMsg, TokensResponse as Cw721TokensResponse};

pub fn query_nft_list(
    querier: &QuerierWrapper<TerraQuery>,
    contract_addr: Addr,
    account_addr: Addr,
) -> StdResult<Cw721TokensResponse> {
    let res: Cw721TokensResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&Cw721QueryMsg::Tokens {
            owner: account_addr.to_string(),
            start_after: None,
            limit: None,
        })?,
    }))?;

    Ok(res)
}

pub fn query_is_nft_holder(
    querier: &QuerierWrapper<TerraQuery>,
    contract_addr: Addr,
    account_addr: Addr,
) -> StdResult<bool> {
    let res: Cw721TokensResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&Cw721QueryMsg::Tokens {
            owner: account_addr.to_string(),
            start_after: None,
            limit: None,
        })?,
    }))?;

    Ok(res.tokens.len() > 0)
}

pub fn query_balance(
    querier: &QuerierWrapper<TerraQuery>,
    account_addr: Addr,
    denom: String,
) -> StdResult<Uint128> {
    // load price form the oracle
    let balance: BalanceResponse = querier.query(&QueryRequest::Bank(BankQuery::Balance {
        address: account_addr.to_string(),
        denom,
    }))?;
    Ok(balance.amount.amount)
}

pub fn query_all_balances(
    querier: &QuerierWrapper<TerraQuery>,
    account_addr: Addr,
) -> StdResult<Vec<Coin>> {
    // load price form the oracle
    let all_balances: AllBalanceResponse =
        querier.query(&QueryRequest::Bank(BankQuery::AllBalances {
            address: account_addr.to_string(),
        }))?;
    Ok(all_balances.amount)
}

pub fn query_token_balance(
    querier: &QuerierWrapper<TerraQuery>,
    contract_addr: Addr,
    account_addr: Addr,
) -> StdResult<Uint128> {
    let res: Cw20BalanceResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&Cw20QueryMsg::Balance {
            address: account_addr.to_string(),
        })?,
    }))?;

    // load balance form the token contract
    Ok(res.balance)
}

pub fn query_token_info(
    querier: &QuerierWrapper<TerraQuery>,
    contract_addr: Addr,
) -> StdResult<TokenInfoResponse> {
    let token_info: TokenInfoResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
    }))?;

    Ok(token_info)
}

pub fn simulate(
    querier: &QuerierWrapper<TerraQuery>,
    classicmoon_contract: Addr,
    offer_asset: &Asset,
) -> StdResult<SimulationResponse> {
    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: classicmoon_contract.to_string(),
        msg: to_binary(&ClassicmoonQueryMsg::Simulation {
            offer_asset: offer_asset.clone(),
        })?,
    }))
}

pub fn reverse_simulate(
    querier: &QuerierWrapper<TerraQuery>,
    classicmoon_contract: Addr,
    ask_asset: &Asset,
) -> StdResult<ReverseSimulationResponse> {
    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: classicmoon_contract.to_string(),
        msg: to_binary(&ClassicmoonQueryMsg::ReverseSimulation {
            ask_asset: ask_asset.clone(),
        })?,
    }))
}

pub fn query_classicmoon_info_from_classicmoon(
    querier: &QuerierWrapper<TerraQuery>,
    classicmoon_contract: Addr,
) -> StdResult<ClassicmoonInfo> {
    let classicmoon_info: ClassicmoonInfo =
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: classicmoon_contract.to_string(),
            msg: to_binary(&ClassicmoonQueryMsg::Classicmoon {})?,
        }))?;

    Ok(classicmoon_info)
}
