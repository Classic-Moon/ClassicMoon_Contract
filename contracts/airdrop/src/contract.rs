use crate::error::ContractError;
use crate::state::{AIRDROP_GLOBAL, AIRDROP_NFT_INFO, AIRDROP_USER_INFO};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128, WasmMsg,
};

use classic_bindings::{TerraMsg, TerraQuery};

use classic_classicmoon::airdrop::{ExecuteMsg, InstantiateMsg, QueryMsg};
use classic_classicmoon::asset::{
    AirdropGlobal, AirdropGlobalRaw, AirdropNftInfo, AirdropUserInfo, AirdropUserInfoResponse,
};
use classic_classicmoon::querier::query_nft_list;
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:airdrop";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const NFT_COLLECTION: &str = "terra15tuwx3r2peluez6sh4yauk4ztry5dg5els4rye534v9n8su5gacs259p77"; // classicmoon nft collection
const AIRDROP_DURATION: u64 = 30 * 86400; // 1 month
const AIRDROP_AMOUNT: Uint128 = Uint128::new(5_100_000_000_000); // airdrop amount per nft is 5.1 million
// const AIRDROP_COUNT_LIMIT: u64 = 20; // 20 months
const AIRDROP_LIMIT_PER_NFT: Uint128 = Uint128::new(20 * 5_100_000_000_000); // total airdrop amount per nft is 20 * 5.1 million

const TERSURY_WALLET: &str = "terra1675g95dpcxulmwgyc0hvf66uxn7vcrr5az2vuk"; // TODO treasury wallet(now prism)
const TOKEN_CONTRACT: &str = "terra1rt0h5502et0tsx7tssl0c8psy3n5lxjvthe3jcgc9d66070zvh7qegu7rk"; // TODO token contract
const AIRDROP_START_TIME: u64 = 1689809000; // TODO timestamp of contract execution

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut<TerraQuery>,
    env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response<TerraMsg>> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let airdrop_config: &AirdropGlobalRaw = &AirdropGlobalRaw {
        total_dropped_amounts: Uint128::zero(),
        last_drop_user: deps.api.addr_canonicalize(env.contract.address.as_str())?,
        last_drop_timestamp: 0,
        last_drop_amount: Uint128::zero(),
    };

    AIRDROP_GLOBAL.save(deps.storage, airdrop_config)?;

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
        ExecuteMsg::Receive(_msg) => {
            return Err(ContractError::Unauthorized {});
        }
        ExecuteMsg::Airdrop {} => airdrop(deps, env, info.sender),
    }
}

// CONTRACT - a user must do token approval
#[allow(clippy::too_many_arguments)]
pub fn airdrop(
    deps: DepsMut<TerraQuery>,
    env: Env,
    sender: Addr,
) -> Result<Response<TerraMsg>, ContractError> {
    let nft_list = query_nft_list(
        &deps.querier,
        Addr::unchecked(NFT_COLLECTION),
        sender.clone(),
    )?
    .tokens;

    if nft_list.len() < 1 {
        return Err(ContractError::NoNftHolder {});
    }

    let current_time = env.block.time.seconds();
    let mut airdrop_amount = Uint128::zero();
    for nft_id in nft_list {
        let mut nft_info: AirdropNftInfo = AIRDROP_NFT_INFO.load(deps.storage, nft_id.clone())?;
        let check_time;
        if nft_info.last_drop_time > AIRDROP_START_TIME {
            check_time = nft_info.last_drop_time;
        } else {
            check_time = AIRDROP_START_TIME
        }
        if current_time > (check_time + AIRDROP_DURATION) {
            let pending_count = (current_time - check_time) / AIRDROP_DURATION;
            let mut pending_amount = AIRDROP_AMOUNT * Uint128::from(pending_count);

            if pending_amount + nft_info.dropped_amount > AIRDROP_LIMIT_PER_NFT {
                pending_amount = AIRDROP_LIMIT_PER_NFT - nft_info.dropped_amount;
            }

            if !pending_amount.is_zero() {
                nft_info.dropped_amount += pending_amount;
                nft_info.last_drop_amount = pending_amount;
                nft_info.last_drop_time = current_time;
                AIRDROP_NFT_INFO.save(deps.storage, nft_id, &nft_info)?;

                airdrop_amount += pending_amount;
            }
        }
    }

    let mut messages: Vec<CosmosMsg<TerraMsg>> = vec![];
    if !airdrop_amount.is_zero() {
        AIRDROP_GLOBAL.update(deps.storage, |mut meta: AirdropGlobalRaw| -> StdResult<_> {
            meta.total_dropped_amounts += airdrop_amount;
            meta.last_drop_user = deps.api.addr_canonicalize(sender.as_str())?;
            meta.last_drop_timestamp = current_time;
            meta.last_drop_amount = airdrop_amount;
            Ok(meta)
        })?;

        let mut user_info = AIRDROP_USER_INFO.load(deps.storage, sender.clone())?;
        user_info.dropped_amount += airdrop_amount;
        user_info.last_drop_amount = airdrop_amount;
        user_info.last_drop_time = current_time;
        AIRDROP_USER_INFO.save(deps.storage, sender.clone(), &user_info)?;

        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: TOKEN_CONTRACT.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: TERSURY_WALLET.to_string(),
                recipient: sender.to_string(),
                amount: airdrop_amount,
            })?,
            funds: vec![],
        }));
    } else {
        return Err(ContractError::NoPendingReward {});
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        ("action", "airdrop"),
        ("receiver", sender.as_str()),
        ("airdrop_amount", &airdrop_amount.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps<TerraQuery>, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::AirdropGlobalInfo {} => Ok(to_binary(&query_airdrop_config(deps)?)?),
        QueryMsg::AirdropNftInfo { token_id } => {
            Ok(to_binary(&query_airdrop_nft_info(deps, token_id)?)?)
        }
        QueryMsg::AirdropUserInfo { account } => {
            Ok(to_binary(&query_airdrop_user_info(deps, env, account)?)?)
        }
    }
}

pub fn query_airdrop_config(deps: Deps<TerraQuery>) -> Result<AirdropGlobal, ContractError> {
    let airdrop_config: AirdropGlobalRaw = AIRDROP_GLOBAL.load(deps.storage)?;
    let airdrop_config: AirdropGlobal = airdrop_config.to_normal(deps.api)?;

    Ok(airdrop_config)
}

pub fn query_airdrop_nft_info(
    deps: Deps<TerraQuery>,
    token_id: String,
) -> Result<AirdropNftInfo, ContractError> {
    let airdrop_nft_info: AirdropNftInfo = AIRDROP_NFT_INFO.load(deps.storage, token_id)?;

    Ok(airdrop_nft_info)
}

pub fn query_airdrop_user_info(
    deps: Deps<TerraQuery>,
    env: Env,
    account: Addr,
) -> Result<AirdropUserInfoResponse, ContractError> {
    let airdrop_user_info: AirdropUserInfo =
        AIRDROP_USER_INFO.load(deps.storage, account.clone())?;

    let nft_list = query_nft_list(&deps.querier, Addr::unchecked(NFT_COLLECTION), account)?.tokens;

    if nft_list.clone().len() < 1 {
        return Err(ContractError::NoNftHolder {});
    }

    let current_time = env.block.time.seconds();
    let mut total_pending_amount = Uint128::zero();
    for nft_id in nft_list.clone() {
        let nft_info: AirdropNftInfo = AIRDROP_NFT_INFO.load(deps.storage, nft_id)?;
        let check_time;
        if nft_info.last_drop_time > AIRDROP_START_TIME {
            check_time = nft_info.last_drop_time;
        } else {
            check_time = AIRDROP_START_TIME
        }
        if current_time > (check_time + AIRDROP_DURATION) {
            let pending_count = (current_time - check_time) / AIRDROP_DURATION;
            let mut pending_amount = AIRDROP_AMOUNT * Uint128::from(pending_count);

            if pending_amount + nft_info.dropped_amount > AIRDROP_LIMIT_PER_NFT {
                pending_amount = AIRDROP_LIMIT_PER_NFT - nft_info.dropped_amount;
            }
            total_pending_amount += pending_amount;
        }
    }

    let mut next_drop_time = env.block.time.seconds() + AIRDROP_DURATION;
    if !total_pending_amount.is_zero() {
        next_drop_time = env.block.time.seconds();
    } else {
        for nft_id in nft_list.clone() {
            let nft_info: AirdropNftInfo = AIRDROP_NFT_INFO.load(deps.storage, nft_id)?;
            if nft_info.last_drop_time < AIRDROP_START_TIME {
                next_drop_time = AIRDROP_START_TIME;
            } else if next_drop_time > (nft_info.last_drop_time + AIRDROP_DURATION) {
                next_drop_time = nft_info.last_drop_time + AIRDROP_DURATION;
            }
        }

        for nft_id in nft_list {
            let nft_info: AirdropNftInfo = AIRDROP_NFT_INFO.load(deps.storage, nft_id)?;
            if !(next_drop_time >= nft_info.last_drop_time + AIRDROP_DURATION) {
                total_pending_amount += AIRDROP_AMOUNT;
            }
        }
    }

    Ok(AirdropUserInfoResponse {
        dropped_amount: airdrop_user_info.dropped_amount,
        last_drop_amount: airdrop_user_info.last_drop_amount,
        last_drop_time: airdrop_user_info.last_drop_time,
        next_drop_time,
        pending_amount: total_pending_amount,
    })
}
