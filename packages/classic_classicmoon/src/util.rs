use classic_bindings::TerraQuery;
use cosmwasm_std::{DepsMut, StdError, StdResult};
use cw2::{get_contract_version, set_contract_version};

pub fn assert_deadline(blocktime: u64, deadline: Option<u64>) -> StdResult<()> {
    if let Some(deadline) = deadline {
        if blocktime >= deadline {
            return Err(StdError::generic_err("Expired deadline"));
        }
    }

    Ok(())
}

pub fn migrate_version(
    deps: DepsMut<TerraQuery>,
    target_contract_version: &str,
    name: &str,
    version: &str,
) -> StdResult<()> {
    let prev_version = get_contract_version(deps.as_ref().storage)?;
    if prev_version.contract != name {
        return Err(StdError::generic_err("invalid contract"));
    }

    if prev_version.version != target_contract_version {
        return Err(StdError::generic_err(format!(
            "invalid contract version. target {}, but source is {}",
            target_contract_version, prev_version.version
        )));
    }

    set_contract_version(deps.storage, name, version)?;

    Ok(())
}

#[test]
fn test_assert_deadline_with_normal() {
    assert_deadline(5u64, Some(10u64)).unwrap();
}

#[test]
fn test_assert_deadline_with_expired() {
    let err = assert_deadline(10u64, Some(5u64)).unwrap_err();
    assert_eq!(err, StdError::generic_err("Expired deadline"))
}

#[test]
fn test_assert_deadline_with_same() {
    let err = assert_deadline(10u64, Some(10u64)).unwrap_err();
    assert_eq!(err, StdError::generic_err("Expired deadline"))
}

#[test]
fn test_assert_deadline_with_none() {
    assert_deadline(5u64, None).unwrap();
}
