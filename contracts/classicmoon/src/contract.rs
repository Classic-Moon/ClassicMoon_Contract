use crate::error::ContractError;
use crate::state::CLASSICMOON_INFO;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Decimal256, Deps,
    DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
    Uint256, WasmMsg,
};

use classic_bindings::{TerraMsg, TerraQuery};

use classic_classicmoon::asset::{Asset, AssetInfo, ClassicmoonInfo, ClassicmoonInfoRaw};
use classic_classicmoon::classicmoon::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, PoolResponse, QueryMsg,
    ReverseSimulationResponse, SimulationResponse,
};
use classic_classicmoon::util::{assert_deadline, migrate_version};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use std::cmp::Ordering;
use std::convert::TryInto;
use std::ops::Mul;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:classicmoon";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const COMMISSION_RATE: u64 = 2; // commission rate = 0.2%
// const DISTRIBUTE_RATE: u64 = 500; // distribute rate = 50%
// const MARKETING_RATE: u64 = 500; // marketing rate = 50%
// const VESTING_DURATION: u64 = 30 * 86400; // 1 month
// const VESTING_COUNT: u64 = 40; // 40 months
// const VESTING_AMOUNT: Uint128 = Uint128::new(680_000_000_000_000_000); // vesting amount = 680 billion
// const AUTOBURN_DURATION: u64 = 10 * 86400; // 10 days
// const CIRCULATING_LIMIT: Uint128 = Uint128::new(10_000_000_000_000_000); // circulating_supply_limit = 10 billion
// const BURN_ABOVE_RATE: u64 = 500; // burn above rate = 50%
// const BURN_BELOW_RATE: u64 = 500; // burn below rate = 1%
// const TERSURY_WALLET: &str = "terra1675g95dpcxulmwgyc0hvf66uxn7vcrr5az2vuk"; // TODO treasury wallet(now prism)
// const BURN_WALLET: &str = "terra1sk06e3dyexuq4shw77y3dsv480xv42mq73anxu"; // burn-listing wallet
// const MARKET_WALLET: &str = "terra1rf76ceh3u0592yd490gucg9kfkvtye3zym95vk"; // marketing-listing wallet
// const START_TIMESTAMP: u64 = 1689773070; // TODO token contract deployed timestamp

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut<TerraQuery>,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response<TerraMsg>> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let classicmoon_info: &ClassicmoonInfoRaw = &ClassicmoonInfoRaw {
        contract_addr: deps.api.addr_canonicalize(env.contract.address.as_str())?,
        liquidity_k_value: Uint128::zero(),
        vesting_epoch: 0,
        autoburn_epoch: 0,
        asset_infos: [
            msg.asset_infos[0].to_raw(deps.api)?,
            msg.asset_infos[1].to_raw(deps.api)?,
        ],
        asset_decimals: msg.asset_decimals,
    };

    CLASSICMOON_INFO.save(deps.storage, classicmoon_info)?;

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
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::ProvideLiquidity {
            assets,
            // receiver,
            deadline,
            slippage_tolerance,
        } => provide_liquidity(
            deps,
            env,
            info,
            assets,
            // receiver,
            deadline,
            slippage_tolerance,
        ),
        ExecuteMsg::Swap {
            offer_asset,
            belief_price,
            max_spread,
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

            swap(
                deps,
                env,
                info.clone(),
                info.sender,
                offer_asset,
                belief_price,
                max_spread,
                to_addr,
                deadline,
            )
        }
    }
}

pub fn receive_cw20(
    deps: DepsMut<TerraQuery>,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response<TerraMsg>, ContractError> {
    let contract_addr = info.sender.clone();

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Swap {
            belief_price,
            max_spread,
            to,
            deadline,
        }) => {
            // only asset contract can execute this message
            let mut authorized: bool = false;
            let config: ClassicmoonInfoRaw = CLASSICMOON_INFO.load(deps.storage)?;
            let pools: [Asset; 2] =
                config.query_pools(&deps.querier, deps.api, env.contract.address.clone())?;
            for pool in pools.iter() {
                if let AssetInfo::Token { contract_addr, .. } = &pool.info {
                    if contract_addr == &info.sender {
                        authorized = true;
                    }
                }
            }

            if !authorized {
                return Err(ContractError::Unauthorized {});
            }

            let to_addr = if let Some(to_addr) = to {
                Some(deps.api.addr_validate(&to_addr).unwrap())
            } else {
                None
            };

            swap(
                deps,
                env,
                info,
                Addr::unchecked(cw20_msg.sender),
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: contract_addr.to_string(),
                    },
                    amount: cw20_msg.amount,
                },
                belief_price,
                max_spread,
                to_addr,
                deadline,
            )
        }
        Ok(Cw20HookMsg::WithdrawLiquidity {
            min_assets,
            deadline,
        }) => {
            let sender_addr = deps.api.addr_validate(&cw20_msg.sender).unwrap();

            withdraw_liquidity(
                deps,
                env,
                info,
                sender_addr,
                cw20_msg.amount,
                min_assets,
                deadline,
            )
        }
        Err(err) => Err(ContractError::Std(err)),
    }
}

/// CONTRACT - should approve contract to use the amount of token
pub fn provide_liquidity(
    deps: DepsMut<TerraQuery>,
    env: Env,
    info: MessageInfo,
    assets: [Asset; 2],
    // receiver: Option<String>,
    deadline: Option<u64>,
    slippage_tolerance: Option<Decimal>,
) -> Result<Response<TerraMsg>, ContractError> {
    assert_deadline(env.block.time.seconds(), deadline)?;

    for asset in assets.iter() {
        asset.assert_sent_native_token_balance(&info)?;
    }

    let classicmoon_info: ClassicmoonInfoRaw = CLASSICMOON_INFO.load(deps.storage)?;
    let mut pools: [Asset; 2] =
        classicmoon_info.query_pools(&deps.querier, deps.api, env.contract.address.clone())?;
    let deposits: [Uint128; 2] = [
        assets
            .iter()
            .find(|a| a.info.equal(&pools[0].info))
            .map(|a| a.amount)
            .expect("Wrong asset info is given"),
        assets
            .iter()
            .find(|a| a.info.equal(&pools[1].info))
            .map(|a| a.amount)
            .expect("Wrong asset info is given"),
    ];

    let mut messages: Vec<CosmosMsg<TerraMsg>> = vec![];
    for (i, pool) in pools.iter_mut().enumerate() {
        if pool.is_native_token() {
            // If the asset is native token, balance is already increased
            // To calculated properly we should subtract user deposit from the pool
            pool.amount = pool.amount.checked_sub(deposits[i])?;
        }
    }

    let total_share = classicmoon_info.liquidity_k_value;
    let share = if total_share.is_zero() {
        // Initial share = collateral amount
        let deposit0: Uint256 = deposits[0].into();
        let deposit1: Uint256 = deposits[1].into();
        let share: Uint128 = match (Decimal256::from_ratio(deposit0.mul(deposit1), 1u8).sqrt()
            * Uint256::from(1u8))
        .try_into()
        {
            Ok(share) => share,
            Err(e) => return Err(ContractError::ConversionOverflowError(e)),
        };

        share
    } else {
        // min(1, 2)
        // 1. sqrt(deposit_0 * exchange_rate_0_to_1 * deposit_0) * (total_share / sqrt(pool_0 * pool_1))
        // == deposit_0 * total_share / pool_0
        // 2. sqrt(deposit_1 * exchange_rate_1_to_0 * deposit_1) * (total_share / sqrt(pool_1 * pool_1))
        // == deposit_1 * total_share / pool_1
        std::cmp::min(
            deposits[0].multiply_ratio(total_share, pools[0].amount),
            deposits[1].multiply_ratio(total_share, pools[1].amount),
        )
    };

    // prevent providing free token
    if share.is_zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    // refund of remaining native token & desired of token
    let mut refund_assets: Vec<Asset> = vec![];
    for (i, pool) in pools.iter().enumerate() {
        let desired_amount = match total_share.is_zero() {
            true => deposits[i],
            false => {
                let mut desired_amount = pool.amount.multiply_ratio(share, total_share);
                if desired_amount.multiply_ratio(total_share, share) != pool.amount {
                    desired_amount += Uint128::from(1u8);
                }

                desired_amount
            }
        };

        let remain_amount = deposits[i] - desired_amount;
        if let Some(slippage_tolerance) = slippage_tolerance {
            if remain_amount > deposits[i] * slippage_tolerance {
                return Err(ContractError::MaxSlippageAssertion {});
            }
        }
        refund_assets.push(Asset {
            info: pool.info.clone(),
            amount: remain_amount,
        });

        if let AssetInfo::NativeToken { denom, .. } = &pool.info {
            if !remain_amount.is_zero() {
                let msg = Asset {
                    amount: remain_amount,
                    info: AssetInfo::NativeToken {
                        denom: denom.to_string(),
                    },
                }
                .into_msg(&deps.querier, info.sender.clone())?;

                messages.push(msg);
            }
        } else if let AssetInfo::Token { contract_addr, .. } = &pool.info {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: env.contract.address.to_string(),
                    amount: desired_amount,
                })?,
                funds: vec![],
            }));
        }
    }

    CLASSICMOON_INFO.update(deps.storage, |mut meta| -> StdResult<_> {
        meta.liquidity_k_value += share;
        Ok(meta)
    })?;

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        ("action", "provide_liquidity"),
        ("sender", info.sender.as_str()),
        ("receiver", env.contract.address.as_str()),
        ("assets", &format!("{}, {}", assets[0], assets[1])),
        ("share", &share.to_string()),
        (
            "refund_assets",
            &format!("{}, {}", refund_assets[0], refund_assets[1]),
        ),
    ]))
}

pub fn withdraw_liquidity(
    deps: DepsMut<TerraQuery>,
    env: Env,
    _info: MessageInfo,
    sender: Addr,
    amount: Uint128,
    min_assets: Option<[Asset; 2]>,
    deadline: Option<u64>,
) -> Result<Response<TerraMsg>, ContractError> {
    assert_deadline(env.block.time.seconds(), deadline)?;

    let classicmoon_info: ClassicmoonInfoRaw = CLASSICMOON_INFO.load(deps.storage)?;

    let pools: [Asset; 2] = classicmoon_info.query_pools(&deps.querier, deps.api, env.contract.address)?;
    let total_share: Uint128 = classicmoon_info.liquidity_k_value;

    let share_ratio: Decimal = Decimal::from_ratio(amount, total_share);
    let refund_assets: Vec<Asset> = pools
        .iter()
        .map(|a| Asset {
            info: a.info.clone(),
            amount: a.amount * share_ratio,
        })
        .collect();

    assert_minimum_assets(refund_assets.to_vec(), min_assets)?;

    // update pool info
    CLASSICMOON_INFO.update(deps.storage, |mut meta| -> StdResult<_> {
        meta.liquidity_k_value = meta.liquidity_k_value.checked_sub(amount)?;
        Ok(meta)
    })?;

    Ok(Response::new()
        .add_messages(vec![
            refund_assets[0]
                .clone()
                .into_msg(&deps.querier, sender.clone())?,
            refund_assets[1]
                .clone()
                .into_msg(&deps.querier, sender.clone())?,
        ])
        .add_attributes(vec![
            ("action", "withdraw_liquidity"),
            ("sender", sender.as_str()),
            ("withdrawn_share", &amount.to_string()),
            (
                "refund_assets",
                &format!("{}, {}", refund_assets[0], refund_assets[1]),
            ),
        ]))
}

// CONTRACT - a user must do token approval
#[allow(clippy::too_many_arguments)]
pub fn swap(
    deps: DepsMut<TerraQuery>,
    env: Env,
    info: MessageInfo,
    sender: Addr,
    offer_asset: Asset,
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
    to: Option<Addr>,
    deadline: Option<u64>,
) -> Result<Response<TerraMsg>, ContractError> {
    assert_deadline(env.block.time.seconds(), deadline)?;

    offer_asset.assert_sent_native_token_balance(&info)?;

    let classicmoon_info: ClassicmoonInfoRaw = CLASSICMOON_INFO.load(deps.storage)?;

    let pools: [Asset; 2] = classicmoon_info.query_pools(&deps.querier, deps.api, env.contract.address)?;

    let offer_pool: Asset;
    let ask_pool: Asset;

    let offer_decimal: u8;
    let ask_decimal: u8;
    // If the asset balance is already increased
    // To calculated properly we should subtract user deposit from the pool
    if offer_asset.info.equal(&pools[0].info) {
        offer_pool = Asset {
            amount: pools[0].amount.checked_sub(offer_asset.amount)?,
            info: pools[0].info.clone(),
        };
        ask_pool = pools[1].clone();

        offer_decimal = classicmoon_info.asset_decimals[0];
        ask_decimal = classicmoon_info.asset_decimals[1];
    } else if offer_asset.info.equal(&pools[1].info) {
        offer_pool = Asset {
            amount: pools[1].amount.checked_sub(offer_asset.amount)?,
            info: pools[1].info.clone(),
        };
        ask_pool = pools[0].clone();

        offer_decimal = classicmoon_info.asset_decimals[1];
        ask_decimal = classicmoon_info.asset_decimals[0];
    } else {
        return Err(ContractError::AssetMismatch {});
    }

    let offer_amount = offer_asset.amount;
    // TODO 0.2% fee check commission_amount
    let (return_amount, spread_amount, commission_amount) =
        compute_swap(offer_pool.amount, ask_pool.amount, offer_amount)?;

    let return_asset = Asset {
        info: ask_pool.info.clone(),
        amount: return_amount,
    };

    // check max spread limit if exist
    assert_max_spread(
        belief_price,
        max_spread,
        offer_asset.clone(),
        return_asset.clone(),
        spread_amount,
        offer_decimal,
        ask_decimal,
    )?;

    // compute tax (0.5% for Native Token by Lunc Policy)
    let tax_amount = return_asset.compute_tax(&deps.querier)?;
    let receiver = to.unwrap_or_else(|| sender.clone());

    let mut messages: Vec<CosmosMsg<TerraMsg>> = vec![];
    if !return_amount.is_zero() {
        messages.push(return_asset.into_msg(&deps.querier, receiver.clone())?);
    }

    // TODO vesting
    // TODO autoburn

    // 1. send collateral token from the contract to a user
    // 2. send inactive commission to collector
    Ok(Response::new().add_messages(messages).add_attributes(vec![
        ("action", "swap"),
        ("sender", sender.as_str()),
        ("receiver", receiver.as_str()),
        ("offer_asset", &offer_asset.info.to_string()),
        ("ask_asset", &ask_pool.info.to_string()),
        ("offer_amount", &offer_amount.to_string()),
        ("return_amount", &return_amount.to_string()),
        ("tax_amount", &tax_amount.to_string()),
        ("spread_amount", &spread_amount.to_string()),
        ("commission_amount", &commission_amount.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps<TerraQuery>, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Classicmoon {} => Ok(to_binary(&query_classicmoon_info(deps)?)?),
        QueryMsg::Pool {} => Ok(to_binary(&query_pool(deps)?)?),
        QueryMsg::Simulation { offer_asset } => {
            Ok(to_binary(&query_simulation(deps, offer_asset)?)?)
        }
        QueryMsg::ReverseSimulation { ask_asset } => {
            Ok(to_binary(&query_reverse_simulation(deps, ask_asset)?)?)
        }
    }
}

pub fn query_classicmoon_info(deps: Deps<TerraQuery>) -> Result<ClassicmoonInfo, ContractError> {
    let classicmoon_info: ClassicmoonInfoRaw = CLASSICMOON_INFO.load(deps.storage)?;
    let classicmoon_info = classicmoon_info.to_normal(deps.api)?;

    Ok(classicmoon_info)
}

pub fn query_pool(deps: Deps<TerraQuery>) -> Result<PoolResponse, ContractError> {
    let classicmoon_info: ClassicmoonInfoRaw = CLASSICMOON_INFO.load(deps.storage)?;
    let contract_addr = deps.api.addr_humanize(&classicmoon_info.contract_addr)?;
    let assets: [Asset; 2] = classicmoon_info.query_pools(&deps.querier, deps.api, contract_addr)?;
    let total_share: Uint128 = classicmoon_info.liquidity_k_value;

    let resp = PoolResponse {
        assets,
        total_share,
    };

    Ok(resp)
}

pub fn query_simulation(
    deps: Deps<TerraQuery>,
    offer_asset: Asset,
) -> Result<SimulationResponse, ContractError> {
    let classicmoon_info: ClassicmoonInfoRaw = CLASSICMOON_INFO.load(deps.storage)?;

    let contract_addr = deps.api.addr_humanize(&classicmoon_info.contract_addr)?;
    let pools: [Asset; 2] = classicmoon_info.query_pools(&deps.querier, deps.api, contract_addr)?;

    let offer_pool: Asset;
    let ask_pool: Asset;
    if offer_asset.info.equal(&pools[0].info) {
        offer_pool = pools[0].clone();
        ask_pool = pools[1].clone();
    } else if offer_asset.info.equal(&pools[1].info) {
        offer_pool = pools[1].clone();
        ask_pool = pools[0].clone();
    } else {
        return Err(ContractError::AssetMismatch {});
    }

    let (return_amount, spread_amount, commission_amount) =
        compute_swap(offer_pool.amount, ask_pool.amount, offer_asset.amount)?;

    Ok(SimulationResponse {
        return_amount,
        spread_amount,
        commission_amount,
    })
}

pub fn query_reverse_simulation(
    deps: Deps<TerraQuery>,
    ask_asset: Asset,
) -> Result<ReverseSimulationResponse, ContractError> {
    let classicmoon_info: ClassicmoonInfoRaw = CLASSICMOON_INFO.load(deps.storage)?;

    let contract_addr = deps.api.addr_humanize(&classicmoon_info.contract_addr)?;
    let pools: [Asset; 2] = classicmoon_info.query_pools(&deps.querier, deps.api, contract_addr)?;

    let offer_pool: Asset;
    let ask_pool: Asset;
    if ask_asset.info.equal(&pools[0].info) {
        ask_pool = pools[0].clone();
        offer_pool = pools[1].clone();
    } else if ask_asset.info.equal(&pools[1].info) {
        ask_pool = pools[1].clone();
        offer_pool = pools[0].clone();
    } else {
        return Err(ContractError::AssetMismatch {});
    }

    let (offer_amount, spread_amount, commission_amount) =
        compute_offer_amount(offer_pool.amount, ask_pool.amount, ask_asset.amount)?;

    Ok(ReverseSimulationResponse {
        offer_amount,
        spread_amount,
        commission_amount,
    })
}

fn compute_swap(
    offer_pool: Uint128,
    ask_pool: Uint128,
    offer_amount: Uint128,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    let offer_pool: Uint256 = offer_pool.into();
    let ask_pool: Uint256 = ask_pool.into();
    let offer_amount: Uint256 = offer_amount.into();

    let commission_rate = Decimal256::permille(COMMISSION_RATE);

    // offer => ask
    // ask_amount = (ask_pool - cp / (offer_pool + offer_amount)) * (1 - commission_rate)
    let return_amount: Uint256 = (ask_pool * offer_amount) / (offer_pool + offer_amount);

    // calculate spread & commission
    let spread_amount: Uint256 =
        (offer_amount * Decimal256::from_ratio(ask_pool, offer_pool)) - return_amount;
    let mut commission_amount: Uint256 = return_amount * commission_rate;
    if return_amount != (commission_amount * (Decimal256::one() / commission_rate)) {
        commission_amount += Uint256::from(1u128);
    }

    // commission will be absorbed to pool
    let return_amount: Uint256 = return_amount - commission_amount;

    Ok((
        return_amount.try_into()?,
        spread_amount.try_into()?,
        commission_amount.try_into()?,
    ))
}

#[test]
fn test_compute_swap_with_huge_pool_variance() {
    let offer_pool = Uint128::from(395451850234u128);
    let ask_pool = Uint128::from(317u128);

    assert_eq!(
        compute_swap(offer_pool, ask_pool, Uint128::from(1u128))
            .unwrap()
            .0,
        Uint128::zero()
    );
}

fn compute_offer_amount(
    offer_pool: Uint128,
    ask_pool: Uint128,
    ask_amount: Uint128,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    let offer_pool: Uint256 = offer_pool.into();
    let ask_pool: Uint256 = ask_pool.into();
    let ask_amount: Uint256 = ask_amount.into();

    let commission_rate = Decimal256::permille(COMMISSION_RATE);

    // ask => offer
    // offer_amount = cp / (ask_pool - ask_amount / (1 - commission_rate)) - offer_pool
    let cp: Uint256 = offer_pool * ask_pool;

    let one_minus_commission = Decimal256::one() - commission_rate;
    let inv_one_minus_commission = Decimal256::one() / one_minus_commission;
    let mut before_commission_deduction: Uint256 = ask_amount * inv_one_minus_commission;
    if before_commission_deduction * one_minus_commission != ask_amount {
        before_commission_deduction += Uint256::one();
    }

    let after_ask_pool = ask_pool - before_commission_deduction;
    let mut after_offer_pool = Uint256::one().multiply_ratio(cp, after_ask_pool);

    if after_offer_pool * (ask_pool - before_commission_deduction) != cp {
        after_offer_pool += Uint256::one();
    }

    let offer_amount: Uint256 = after_offer_pool - offer_pool;

    let before_spread_deduction: Uint256 =
        offer_amount * Decimal256::from_ratio(ask_pool, offer_pool);

    let spread_amount = if before_spread_deduction > before_commission_deduction {
        before_spread_deduction - before_commission_deduction
    } else {
        Uint256::zero()
    };

    let commission_amount = before_commission_deduction - ask_amount;

    Ok((
        offer_amount.try_into()?,
        spread_amount.try_into()?,
        commission_amount.try_into()?,
    ))
}

/// If `belief_price` and `max_spread` both are given,
/// we compute new spread else we just use classicmoon
/// spread to check `max_spread`
pub fn assert_max_spread(
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
    offer_asset: Asset,
    return_asset: Asset,
    spread_amount: Uint128,
    offer_decimal: u8,
    return_decimal: u8,
) -> Result<(), ContractError> {
    let (offer_amount, return_amount, spread_amount): (Uint256, Uint256, Uint256) =
        match offer_decimal.cmp(&return_decimal) {
            Ordering::Greater => {
                let diff_decimal = 10u64.pow((offer_decimal - return_decimal).into());

                (
                    offer_asset.amount.into(),
                    return_asset
                        .amount
                        .checked_mul(Uint128::from(diff_decimal))?
                        .into(),
                    spread_amount
                        .checked_mul(Uint128::from(diff_decimal))?
                        .into(),
                )
            }
            Ordering::Less => {
                let diff_decimal = 10u64.pow((return_decimal - offer_decimal).into());

                (
                    offer_asset
                        .amount
                        .checked_mul(Uint128::from(diff_decimal))?
                        .into(),
                    return_asset.amount.into(),
                    spread_amount.into(),
                )
            }
            Ordering::Equal => (
                offer_asset.amount.into(),
                return_asset.amount.into(),
                spread_amount.into(),
            ),
        };

    if let (Some(max_spread), Some(belief_price)) = (max_spread, belief_price) {
        let belief_price: Decimal256 = belief_price.into();
        let max_spread: Decimal256 = max_spread.into();

        let expected_return = offer_amount * (Decimal256::one() / belief_price);
        let spread_amount = if expected_return > return_amount {
            expected_return - return_amount
        } else {
            Uint256::zero()
        };

        if return_amount < expected_return
            && Decimal256::from_ratio(spread_amount, expected_return) > max_spread
        {
            return Err(ContractError::MaxSpreadAssertion {});
        }
    } else if let Some(max_spread) = max_spread {
        let max_spread: Decimal256 = max_spread.into();
        if Decimal256::from_ratio(spread_amount, return_amount + spread_amount) > max_spread {
            return Err(ContractError::MaxSpreadAssertion {});
        }
    }

    Ok(())
}

pub fn assert_minimum_assets(
    assets: Vec<Asset>,
    min_assets: Option<[Asset; 2]>,
) -> Result<(), ContractError> {
    if let Some(min_assets) = min_assets {
        min_assets.iter().try_for_each(|min_asset| {
            match assets.iter().find(|asset| asset.info == min_asset.info) {
                Some(asset) => {
                    if asset.amount.cmp(&min_asset.amount).is_lt() {
                        return Err(ContractError::MinAmountAssertion {
                            min_asset: min_asset.to_string(),
                            asset: asset.to_string(),
                        });
                    }
                }
                None => {
                    return Err(ContractError::MinAmountAssertion {
                        min_asset: min_asset.to_string(),
                        asset: Asset {
                            info: min_asset.info.clone(),
                            amount: Uint128::zero(),
                        }
                        .to_string(),
                    })
                }
            };

            Ok(())
        })?;
    }

    Ok(())
}

const TARGET_CONTRACT_VERSION: &str = "0.1.1";
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(
    deps: DepsMut<TerraQuery>,
    _env: Env,
    _msg: MigrateMsg,
) -> Result<Response<TerraMsg>, ContractError> {
    migrate_version(
        deps,
        TARGET_CONTRACT_VERSION,
        CONTRACT_NAME,
        CONTRACT_VERSION,
    )?;

    Ok(Response::default())
}
