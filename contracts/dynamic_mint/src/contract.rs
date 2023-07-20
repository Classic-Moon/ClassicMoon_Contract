use crate::error::ContractError;
use crate::state::DYNAMIC_INFO;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Decimal256, Deps, DepsMut, Env,
    MessageInfo, Response, StdResult, Uint128, Uint256, WasmMsg,
};

use classic_bindings::{TerraMsg, TerraQuery};

use classic_classicmoon::asset::{Asset, AssetInfo, DynamicInfo, DynamicInfoRaw};
use classic_classicmoon::dynamic::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, PoolResponse, QueryMsg,
    ReverseSimulationResponse, SimulationResponse,
};
use classic_classicmoon::querier::{query_balance, query_is_nft_holder, query_token_balance};
use classic_classicmoon::util::{assert_deadline, migrate_version};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use std::cmp::Ordering;
use std::convert::TryInto;
use std::ops::Mul;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:dynamic-mint";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const CIRCULATING_LIMIT: Uint128 = Uint128::new(200_000_000_000_000_000); // circulating_supply_limit = 200 billion
const DISCOUNT_RATE: u64 = 950; // discount rate = 5%, 1000 - 50 = 950
const DISCOUNT_DENOMINATOR: u64 = 1000; // denominator = 1000

const LUNC_USTC_PAIR: &str = "terra1sgu6yca6yjk0a34l86u6ju4apjcd6refwuhgzv"; // lunc-ustc pool of loop_factory
const CLASSICMOON_COLLECTION: &str =
    "terra15tuwx3r2peluez6sh4yauk4ztry5dg5els4rye534v9n8su5gacs259p77"; // classicmoon nft collection
const FURY_COLLECTION: &str = "terra1g7we2dvzgyfyh39zg44n0rlyh9xh4ym9z0wut7"; // fury nft collection

const BURN_WALLET: &str = "terra1sk06e3dyexuq4shw77y3dsv480xv42mq73anxu"; // burn-listing wallet
const TERSURY_WALLET: &str = "terra1675g95dpcxulmwgyc0hvf66uxn7vcrr5az2vuk"; // TODO treasury wallet(now prism)

const TOKEN_CONTRACT: &str = "terra1rt0h5502et0tsx7tssl0c8psy3n5lxjvthe3jcgc9d66070zvh7qegu7rk"; // TODO token contract
const MOON_CONTRACT: &str = "terra1ffx3j5w2sf6yqysmyyhl2d4j80wxw9k3yxe3exleyjapqccxdg7sny4j8c"; // TODO classicmoon contract

const START_MINT_BY_LUNC: u64 = 1689809000 + 60 * 86400; // TODO 2 months later from the date of contract execution
const START_MINT_BY_USTC: u64 = 1689809000 + 90 * 86400; // TODO 3 months later from the date of contract execution

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut<TerraQuery>,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response<TerraMsg>> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let dynamic_info: &DynamicInfoRaw = &DynamicInfoRaw {
        totalLuncBurnAmount: Uint128::zero(),
        totalUstcBurnAmount: Uint128::zero(),
        totalMintedClsmAmount: Uint128::zero(),
    };

    DYNAMIC_INFO.save(deps.storage, dynamic_info)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut<TerraQuery>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response<TerraMsg>, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => {
            return Err(ContractError::Unauthorized {});
        }
        ExecuteMsg::Mint {
            offer_asset,
            to,
            deadline,
        } => {
            if !offer_asset.is_native_token() {
                return Err(ContractError::Unauthorized {});
            }

            let to_addr = if let Some(to_addr) = to {
                Some(deps.api.addr_validate(&to_addr).unwrap())
            } else {
                None
            };

            mint(
                deps,
                env,
                info.clone(),
                info.sender,
                offer_asset,
                to_addr,
                deadline,
            )
        }
    }
}

// CONTRACT - a user must do token approval
#[allow(clippy::too_many_arguments)]
pub fn mint(
    deps: DepsMut<TerraQuery>,
    env: Env,
    info: MessageInfo,
    sender: Addr,
    offer_asset: Asset,
    to: Option<Addr>,
    deadline: Option<u64>,
) -> Result<Response<TerraMsg>, ContractError> {
    assert_deadline(env.block.time.seconds(), deadline)?;

    offer_asset.assert_sent_native_token_balance(&info)?;

    if !query_is_nft_holder(
        &deps.querier,
        Addr::unchecked(CLASSICMOON_COLLECTION),
        sender.clone(),
    )? {
        return Err(ContractError::NoNftHolder {});
    }

    if !query_is_nft_holder(
        &deps.querier,
        Addr::unchecked(FURY_COLLECTION),
        sender.clone(),
    )? {
        return Err(ContractError::NoNftHolder {});
    }

    let mint_amount;

    match offer_asset.info.clone() {
        AssetInfo::Token { .. } => {
            return Err(ContractError::AssetMismatch {});
        }
        AssetInfo::NativeToken { denom, .. } => {
            let token_balance = query_token_balance(
                &deps.querier,
                Addr::unchecked(TOKEN_CONTRACT),
                Addr::unchecked(MOON_CONTRACT),
            )?;
            let lunc_balance = query_balance(
                &deps.querier,
                Addr::unchecked(MOON_CONTRACT),
                "uluna".to_string(),
            )?;

            if denom == "uluna" {
                if env.block.time.seconds() < START_MINT_BY_LUNC {
                    return Err(ContractError::InLockTimeToMint {});
                }

                mint_amount =
                    compute_mint_by_lunc(token_balance, lunc_balance, offer_asset.amount)?;

                DYNAMIC_INFO.update(deps.storage, |mut meta: DynamicInfoRaw| -> StdResult<_> {
                    meta.totalLuncBurnAmount += offer_asset.amount;
                    meta.totalMintedClsmAmount += mint_amount;
                    Ok(meta)
                })?;
            } else if denom == "uusd" {
                if env.block.time.seconds() < START_MINT_BY_USTC {
                    return Err(ContractError::InLockTimeToMint {});
                }

                let pool_lunc = query_balance(
                    &deps.querier,
                    Addr::unchecked(LUNC_USTC_PAIR),
                    "uluna".to_string(),
                )?;
                let pool_ustc = query_balance(
                    &deps.querier,
                    Addr::unchecked(LUNC_USTC_PAIR),
                    "uusd".to_string(),
                )?;

                mint_amount = compute_mint_by_ustc(
                    token_balance,
                    lunc_balance,
                    pool_lunc,
                    pool_ustc,
                    offer_asset.amount,
                )?;

                DYNAMIC_INFO.update(deps.storage, |mut meta: DynamicInfoRaw| -> StdResult<_> {
                    meta.totalUstcBurnAmount += offer_asset.amount;
                    meta.totalMintedClsmAmount += mint_amount;
                    Ok(meta)
                })?;
            } else {
                return Err(ContractError::AssetMismatch {});
            }
        }
    }

    let receiver = to.unwrap_or_else(|| sender.clone());

    let mut messages: Vec<CosmosMsg<TerraMsg>> = vec![];
    if !mint_amount.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: TOKEN_CONTRACT.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: receiver.to_string(),
                amount: mint_amount,
            })?,
            funds: vec![],
        }));
    }

    // compute tax (0.5% for Native Token by Lunc Policy)
    let tax_amount = offer_asset.compute_tax(&deps.querier)?;
    if !offer_asset.amount.clone().is_zero() {
        messages.push(offer_asset.clone().into_msg(&deps.querier, Addr::unchecked(BURN_WALLET))?);
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        ("action", "mint"),
        ("sender", sender.as_str()),
        ("receiver", receiver.as_str()),
        ("offer_asset", &offer_asset.info.to_string()),
        ("offer_amount", &offer_asset.amount.to_string()),
        ("tax_amount", &tax_amount.to_string()),
        ("mint_amount", &mint_amount.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps<TerraQuery>, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Dynamic {} => Ok(to_binary(&query_dynamic_info(deps)?)?),
        QueryMsg::IsMintableByLunc { account } => {
            Ok(to_binary(&query_is_mintable_by_lunc(deps, env, account)?)?)
        }
        QueryMsg::IsMintableByUstc { account } => {
            Ok(to_binary(&query_is_mintable_by_ustc(deps, env, account)?)?)
        }
        QueryMsg::GetAmountMint { offer_asset } => {
            Ok(to_binary(&query_get_amount_mint(deps, offer_asset)?)?)
        }
        QueryMsg::GetAmountLunc { mint_amount } => {
            Ok(to_binary(&query_get_amount_lunc(deps, mint_amount)?)?)
        }
        QueryMsg::GetAmountUstc { mint_amount } => {
            Ok(to_binary(&query_get_amount_ustc(deps, mint_amount)?)?)
        }
    }
}

pub fn query_dynamic_info(deps: Deps<TerraQuery>) -> Result<DynamicInfo, ContractError> {
    let dynamic_info: DynamicInfoRaw = DYNAMIC_INFO.load(deps.storage)?;
    let dynamic_info: DynamicInfo = dynamic_info.to_normal(deps.api)?;

    Ok(dynamic_info)
}

pub fn query_is_mintable_by_lunc(
    deps: Deps<TerraQuery>,
    env: Env,
    account: Addr,
) -> Result<Uint128, ContractError> {
    let is_classicmoon_holder = query_is_nft_holder(
        &deps.querier,
        Addr::unchecked(CLASSICMOON_COLLECTION),
        account.clone(),
    )?;
    let is_fury_holder =
        query_is_nft_holder(&deps.querier, Addr::unchecked(FURY_COLLECTION), account)?;

    if !is_classicmoon_holder & !is_fury_holder {
        return Err(ContractError::NoNftHolder {});
    }

    if env.block.time.seconds() > START_MINT_BY_LUNC {
        return Ok(Uint128::zero());
    }

    Ok(Uint128::from(START_MINT_BY_LUNC - env.block.time.seconds())) // remain time to start
}

pub fn query_is_mintable_by_ustc(
    deps: Deps<TerraQuery>,
    env: Env,
    account: Addr,
) -> Result<Uint128, ContractError> {
    let is_classicmoon_holder = query_is_nft_holder(
        &deps.querier,
        Addr::unchecked(CLASSICMOON_COLLECTION),
        account.clone(),
    )?;
    let is_fury_holder =
        query_is_nft_holder(&deps.querier, Addr::unchecked(FURY_COLLECTION), account)?;

    if !is_classicmoon_holder & !is_fury_holder {
        return Err(ContractError::NoNftHolder {});
    }

    if env.block.time.seconds() > START_MINT_BY_USTC {
        return Ok(Uint128::zero());
    }

    Ok(Uint128::from(START_MINT_BY_USTC - env.block.time.seconds())) // remain time to start
}

pub fn query_get_amount_mint(
    deps: Deps<TerraQuery>,
    offer_asset: Asset,
) -> Result<Uint128, ContractError> {
    let token_bal = query_token_balance(
        &deps.querier,
        Addr::unchecked(TOKEN_CONTRACT),
        Addr::unchecked(MOON_CONTRACT),
    )?;
    let lunc_bal = query_balance(
        &deps.querier,
        Addr::unchecked(MOON_CONTRACT),
        "uluna".to_string(),
    )?;

    match offer_asset.info {
        AssetInfo::Token { .. } => {
            return Err(ContractError::AssetMismatch {});
        }
        AssetInfo::NativeToken { denom, .. } => {
            if denom == "uluna" {
                return Ok(compute_mint_by_lunc(
                    token_bal,
                    lunc_bal,
                    offer_asset.amount,
                )?);
            } else if denom == "uusd" {
                let pool_lunc = query_balance(
                    &deps.querier,
                    Addr::unchecked(LUNC_USTC_PAIR),
                    "uluna".to_string(),
                )?;
                let pool_ustc = query_balance(
                    &deps.querier,
                    Addr::unchecked(LUNC_USTC_PAIR),
                    "uusd".to_string(),
                )?;
                return Ok(compute_mint_by_ustc(
                    token_bal,
                    lunc_bal,
                    pool_lunc,
                    pool_ustc,
                    offer_asset.amount,
                )?);
            } else {
                return Err(ContractError::AssetMismatch {});
            }
        }
    }
}

pub fn query_get_amount_lunc(
    deps: Deps<TerraQuery>,
    mint_amount: Uint128,
) -> Result<Uint128, ContractError> {
    let token_bal = query_token_balance(
        &deps.querier,
        Addr::unchecked(TOKEN_CONTRACT),
        Addr::unchecked(MOON_CONTRACT),
    )?;
    let lunc_bal = query_balance(
        &deps.querier,
        Addr::unchecked(MOON_CONTRACT),
        "uluna".to_string(),
    )?;
    Ok(compute_lunc_by_mint(token_bal, lunc_bal, mint_amount)?)
}

pub fn query_get_amount_ustc(
    deps: Deps<TerraQuery>,
    mint_amount: Uint128,
) -> Result<Uint128, ContractError> {
    let token_bal = query_token_balance(
        &deps.querier,
        Addr::unchecked(TOKEN_CONTRACT),
        Addr::unchecked(MOON_CONTRACT),
    )?;
    let lunc_bal = query_balance(
        &deps.querier,
        Addr::unchecked(MOON_CONTRACT),
        "uluna".to_string(),
    )?;
    let pool_lunc = query_balance(
        &deps.querier,
        Addr::unchecked(LUNC_USTC_PAIR),
        "uluna".to_string(),
    )?;
    let pool_ustc = query_balance(
        &deps.querier,
        Addr::unchecked(LUNC_USTC_PAIR),
        "uusd".to_string(),
    )?;
    Ok(compute_ustc_by_mint(
        token_bal,
        lunc_bal,
        pool_lunc,
        pool_ustc,
        mint_amount,
    )?)
}

fn compute_lunc_by_mint(
    token_balance: Uint128,
    lunc_balance: Uint128,
    mint_amount: Uint128,
) -> StdResult<Uint128> {
    let token_balance: Uint256 = token_balance.into();
    let lunc_balance: Uint256 = lunc_balance.into();
    let mint_amount: Uint256 = mint_amount.into();

    let eval_amount: Uint256 = mint_amount * lunc_balance / token_balance;
    let ret_amount: Uint256 =
        eval_amount * Uint256::from(DISCOUNT_RATE) / Uint256::from(DISCOUNT_DENOMINATOR);

    Ok(ret_amount.try_into()?)
}

fn compute_ustc_by_mint(
    token_balance: Uint128,
    lunc_balance: Uint128,
    pool_lunc: Uint128,
    pool_ustc: Uint128,
    mint_amount: Uint128,
) -> StdResult<Uint128> {
    let token_balance: Uint256 = token_balance.into();
    let lunc_balance: Uint256 = lunc_balance.into();
    let pool_lunc: Uint256 = pool_lunc.into();
    let pool_ustc: Uint256 = pool_ustc.into();
    let mint_amount: Uint256 = mint_amount.into();

    let lunc_amount: Uint256 = mint_amount * lunc_balance / token_balance;

    let eval_amount: Uint256 = lunc_amount * pool_ustc / pool_lunc;
    let ret_amount: Uint256 =
        eval_amount * Uint256::from(DISCOUNT_RATE) / Uint256::from(DISCOUNT_DENOMINATOR);

    Ok(ret_amount.try_into()?)
}

fn compute_mint_by_lunc(
    token_balance: Uint128,
    lunc_balance: Uint128,
    offer_lunc: Uint128,
) -> StdResult<Uint128> {
    let token_balance: Uint256 = token_balance.into();
    let lunc_balance: Uint256 = lunc_balance.into();
    let offer_lunc: Uint256 = offer_lunc.into();

    let eval_amount: Uint256 = offer_lunc * token_balance / lunc_balance;
    let ret_amount: Uint256 =
        eval_amount * Uint256::from(DISCOUNT_DENOMINATOR) / Uint256::from(DISCOUNT_RATE);

    Ok(ret_amount.try_into()?)
}

fn compute_mint_by_ustc(
    token_balance: Uint128,
    lunc_balance: Uint128,
    pool_lunc: Uint128,
    pool_ustc: Uint128,
    offer_ustc: Uint128,
) -> StdResult<Uint128> {
    let token_balance: Uint256 = token_balance.into();
    let lunc_balance: Uint256 = lunc_balance.into();
    let pool_lunc: Uint256 = pool_lunc.into();
    let pool_ustc: Uint256 = pool_ustc.into();
    let offer_ustc: Uint256 = offer_ustc.into();

    let offer_lunc: Uint256 = offer_ustc * pool_lunc / pool_ustc;

    let eval_amount: Uint256 = offer_lunc * token_balance / lunc_balance;
    let ret_amount: Uint256 =
        eval_amount * Uint256::from(DISCOUNT_DENOMINATOR) / Uint256::from(DISCOUNT_RATE);

    Ok(ret_amount.try_into()?)
}
