NETWORK=mainnet
FUNCTION=$1
CATEGORY=$2
PARAM_1=$3
PARAM_2=$4
PARAM_3=$5
ADDR_PRISM="terra1675g95dpcxulmwgyc0hvf66uxn7vcrr5az2vuk"
ADDR_PRISM2="terra1tvlszuvjud7ckguglcmzdyh8wx9g0wy5ujhy0h"

case $NETWORK in
 devnet)
 NODE=""
 DENOM=""
 CHAIN_ID=""
 WALLET=""
 ADDR_ADMIN=$ADDR_PRISM
 GAS=0.001
 ;;
 testnet)
 NODE=""
 DENOM=""
 CHAIN_ID=rebel-2
 WALLET=""
 ADDR_ADMIN=$ADDR_PRISM
 GAS=0.001
 ;;
 mainnet)
NODE="https://terra-classic-rpc.publicnode.com:443"
# NODE="https://terra-rpc.easy2stake.com:443"
# NODE="https://terra.stakesystems.io:2053"
# NODE="https://terra-node.mcontrol.ml"
# NODE="http://public-node.terra.dev:26657"
# NODE="http://172.104.133.249:26657"
# NODE="http://93.66.103.120:26657"
# NODE="https://rpc-terra.synergynodes.com:443/"
 DENOM=uluna
 CHAIN_ID=columbus-5
 WALLET="--from prism"
 WALLET2="--from prism2"
 ADDR_ADMIN=$ADDR_PRISM
 GAS=0.001
 ;; 
esac

NODECHAIN="--node $NODE --chain-id $CHAIN_ID"
TXFLAG=" $NODECHAIN --gas=auto --gas-adjustment=1.5 --gas-prices=50uluna --broadcast-mode=block --keyring-backend test -y"

RELEASE_DIR="release/"
INFO_DIR="info/"
INFONET_DIR=$INFO_DIR$NETWORK"/"
CODE_DIR=$INFONET_DIR"code/"
ADDRESS_DIR=$INFONET_DIR"address/"
CONTRACT_DIR="contracts/"
LIBRARY_DIR="libraries/"
[ ! -d $RELEASE_DIR ] && mkdir $RELEASE_DIR
[ ! -d $INFO_DIR ] &&mkdir $INFO_DIR
[ ! -d $INFONET_DIR ] &&mkdir $INFONET_DIR
[ ! -d $CODE_DIR ] &&mkdir $CODE_DIR
[ ! -d $ADDRESS_DIR ] &&mkdir $ADDRESS_DIR

CLASSICMOON="classicmoon"
CLASSICMOON_TOKEN="classicmoon_token"
DYNAMIC_MINT="dynamic_mint"
AIRDROP="airdrop"

##############################################
### ENV, Build, Upload, Instantiate, Clean ###
##############################################

CreateEnv() {
    sudo apt-get update && sudo apt upgrade -y
    sudo apt-get install make build-essential gcc git jq chrony -y
    wget https://golang.org/dl/go1.18.1.linux-amd64.tar.gz
    sudo tar -C /usr/local -xzf go1.18.1.linux-amd64.tar.gz
    rm -rf go1.18.1.linux-amd64.tar.gz

    export GOROOT=/usr/local/go
    export GOPATH=$HOME/go
    export GO111MODULE=on
    export PATH=$PATH:/usr/local/go/bin:$HOME/go/bin
    
    rustup default stable
    rustup target add wasm32-unknown-unknown

    # git clone https://github.com/terra-money/classic-core/
    # cd classic-core
    # git fetch
    # git checkout release/v0.6.x
    # make install
    # cd ../
    # rm -rf classic-core
    git clone https://github.com/classic-terra/core/
    cd core
    git fetch
    # git checkout release/v1.1.x
    git checkout main
    make install
    cd ../
    rm -rf core
}

RustBuild() {
    echo "================================================="
    echo "Rust Optimize Build Start"
    
    rm -rf target
    
    cd contracts
    
    cd $CATEGORY
    RUSTFLAGS='-C link-arg=-s' cargo wasm
    cd ../../

    cp target/wasm32-unknown-unknown/release/$CATEGORY.wasm release/
}

Upload() {
    echo "================================================="
    echo "Build $RELEASE_DIR$CATEGORY"
    
    cd contracts
    
    cd $CATEGORY
    RUSTFLAGS='-C link-arg=-s' cargo wasm    
    
    cd ../../
    cp target/wasm32-unknown-unknown/release/$CATEGORY.wasm release/
    sleep 3

    echo "-------------------------------------------------"
    echo "Upload $RELEASE_DIR$CATEGORY"

    echo "terrad tx wasm store $RELEASE_DIR$CATEGORY".wasm" $WALLET $TXFLAG --output json | jq -r '.txhash'"
    UPLOADTX=$(terrad tx wasm store $RELEASE_DIR$CATEGORY".wasm" $WALLET $TXFLAG --output json | jq -r '.txhash')

    echo "Upload txHash: "$UPLOADTX
    echo "================================================="
    echo "GetCode"
	
    CODE_ID=""
    while [[ $CODE_ID == "" ]]
    do 
        sleep 3
        CODE_ID=$(terrad query tx $UPLOADTX $NODECHAIN --output json | jq -r '.logs[0].events[-1].attributes[1].value')
    done
    echo "Contract Code_id: "$CODE_ID
    # save to FILE_CODE_ID
    echo $CODE_ID > $CODE_DIR$CATEGORY
}

RemoveHistory() {
    rm -rf release
    rm -rf target
    rm -rf info
}

BatchUpload() {
    echo "======================BatchUpload Start======================"
    
    # CATEGORY=$CLASSICMOON_TOKEN
    # printf "y\n" | Upload
    # sleep 3
    
    # CATEGORY=$CLASSICMOON
    # printf "y\n" | Upload
    # sleep 3

    CATEGORY=$DYNAMIC_MINT
    printf "y\n" | Upload
    sleep 3

    CATEGORY=$AIRDROP
    printf "y\n" | Upload
    sleep 3

    echo "======================BatchUpload End======================"
}

Instantiate() {
    echo "================================================="
    echo "Instantiate Contract "$CATEGORY
    #read from FILE_CODE_ID
    CODE_ID=$(cat $CODE_DIR$CATEGORY)
    echo "Code id: " $CODE_ID

    MSG=$PARAM_1
    LABEL=$PARAM_2
    
    TXHASH=$(terrad tx wasm instantiate $CODE_ID "$MSG" --label $LABEL --admin $ADDR_ADMIN $WALLET $TXFLAG --output json | jq -r '.txhash')
    echo $TXHASH
    CONTRACT_ADDR=""
    while [[ $CONTRACT_ADDR == "" ]]
    do
        sleep 3
        CONTRACT_ADDR=$(terrad query tx $TXHASH $NODECHAIN --output json | jq -r '.logs[0].events[0].attributes[0].value')
    done
    echo "Contract Address: " $CONTRACT_ADDR
    echo $CONTRACT_ADDR > $ADDRESS_DIR$CATEGORY
}

BatchInstantiate() {
    echo "======================BatchInstantiate Start======================"

    # CATEGORY=$CLASSICMOON_TOKEN
    # PARAM_1='{"name":"ClassicMoon Test", "symbol":"TCLSM", "decimals":6, "initial_balances":[{"address":"'$ADDR_ADMIN'", "amount":"6800000000000000000"}], "mint":{"minter":"'$ADDR_ADMIN'"}, "marketing":{"marketing":"'$ADDR_ADMIN'","logo":{"url":"https://classicmoon-frontend-2023.web.app/logo83.png"}}}'
    # PARAM_2="TCLSM"
    # printf "y\n" | Instantiate
    # sleep 3

    # CATEGORY=$CLASSICMOON
    # PARAM_1='{"asset_infos":[{"token":{"contract_addr":"'$(cat $ADDRESS_DIR$CLASSICMOON_TOKEN)'"}}, {"native_token":{"denom":"uluna"}}], "token_code_id":'$(cat $CODE_DIR$CLASSICMOON_TOKEN)', "asset_decimals":[6, 6]}'
    # PARAM_2="ClassicMoon"
    # printf "y\n" | Instantiate
    # sleep 3

    CATEGORY=$DYNAMIC_MINT
    PARAM_1='{}'
    PARAM_2="Dynamic_mint"
    printf "y\n" | Instantiate
    sleep 3

    CATEGORY=$AIRDROP
    PARAM_1='{}'
    PARAM_2="Airdrop"
    printf "y\n" | Instantiate
    sleep 3

    echo "======================BatchInstantiate End======================"
}

##############################################
##################   Util   ##################
##############################################

Balances() {
    ############### prism ###############
    echo prism lunc balance
    printf "y\n" | terrad query bank balances $ADDR_PRISM $NODECHAIN --output json
    sleep 3

    echo prism CLSM balance
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) '{"balance":{"address":"'$ADDR_PRISM'"}}' $NODECHAIN --output json
    sleep 3

    ############### prism2 ###############
    echo prism2 lunc balance
    printf "y\n" | terrad query bank balances $ADDR_PRISM2 $NODECHAIN --output json
    sleep 3

    echo prism2 CLSM balance
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) '{"balance":{"address":"'$ADDR_PRISM2'"}}' $NODECHAIN --output json
    sleep 3

    ############### classicmoon ###############
    echo classicmoon lunc balance
    printf "y\n" | terrad query bank balances $(cat $ADDRESS_DIR$CLASSICMOON) $NODECHAIN --output json
    sleep 3

    echo classicmoon CLSM balance
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) '{"balance":{"address":"'$(cat $ADDRESS_DIR$CLASSICMOON)'"}}' $NODECHAIN --output json
    sleep 3
}

##############################################
##################   Token   #################
##############################################

TokenMintByPrism() {
    echo "================================================="
    echo "TokenMintByPrism"
    PARAM_1='{"mint": {"recipient": "'$ADDR_PRISM'", "amount": "1000000000000" }}'
    echo "terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG"
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
}

UpdateMinterAsPrism2() {
    echo "================================================="
    echo "UpdateMinterAsPrism2"
    PARAM_1='{"update_minter": {"new_minter": "'$ADDR_PRISM2'" }}'
    echo "terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG"
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
}

TokenMintByPrism2() {
    echo "================================================="
    echo "TokenMintByPrism2"
    PARAM_1='{"mint": {"recipient": "'$ADDR_PRISM2'", "amount": "1000000000000" }}'
    echo "terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET2 $TXFLAG"
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET2 $TXFLAG
    sleep 5
}

UpdateMinterAsPrism() {
    echo "================================================="
    echo "UpdateMinterAsPrism"
    PARAM_1='{"update_minter": {"new_minter": "'$ADDR_PRISM'" }}'
    echo "terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET2 $TXFLAG"
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET2 $TXFLAG
    sleep 5
}

IncreaseAllowance() {
    echo "================================================="
    echo "Increase Allowance"
    PARAM_1='{"increase_allowance": {"spender": "'$(cat $ADDRESS_DIR$CLASSICMOON)'", "amount": "100000000000", "expires": {"never": {}}}}'
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
    echo "End"
}

UpdateMinterAsDynamicMinter() {
    echo "================================================="
    echo "UpdateMinterAsDynamicMinter"
    PARAM_1='{"update_minter": {"new_minter": "'$(cat $ADDRESS_DIR$DYNAMIC_MINT)'" }}'
    echo "terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG"
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
}

IncreaseAllowanceForClassicMoon() {
    echo "================================================="
    echo "IncreaseAllowanceForClassicMoon"
    PARAM_1='{"increase_allowance": {"spender": "'$(cat $ADDRESS_DIR$CLASSICMOON)'", "amount": "6800000000000000000", "expires": {"never": {}}}}'
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
    echo "End"
}

IncreaseAllowanceForAirdrop() {
    echo "================================================="
    echo "IncreaseAllowanceForAirdrop"
    PARAM_1='{"increase_allowance": {"spender": "'$(cat $ADDRESS_DIR$AIRDROP)'", "amount": "6800000000000000000", "expires": {"never": {}}}}'
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
    echo "End"
}

TokenTransfer () {
    echo "================================================="
    echo "Start TokenTransfer"
    PARAM_1='{"transfer": {"recipient": "'$ADDR_PRISM2'", "amount": "1000000000" }}'
    PARAM_2='TCLSM'
    echo "terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG"
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
    echo "End"
}

GetAllowance() {
    echo "================================================="
    echo "Allowance"
    PARAM_1='{"allowance": {"owner": "'$ADDR_ADMIN'", "spender": "'$(cat $ADDRESS_DIR$CLASSICMOON)'"}}'
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $NODECHAIN --output json
    sleep 5
    echo "End"
}

##############################################
######        CLASSICMOON Execute       ######
##############################################

# 100K CLSM : 100 LUNA
AddLiquidity() {
    echo "================================================="
    echo "Start Add Liquidity"
    PARAM_1='{"provide_liquidity": {"assets": [{"info": {"token":{"contract_addr":"'$(cat $ADDRESS_DIR$CLASSICMOON_TOKEN)'"}}, "amount": "100000000000"}, {"info": {"native_token":{"denom":"uluna"}}, "amount": "100000000"}]}}'
    echo "terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON) "$PARAM_1" --amount 100000000uluna $WALLET $TXFLAG"
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON) "$PARAM_1" --amount 100000000uluna $WALLET $TXFLAG
    sleep 5
    echo "End"
}

SwapLuncToClsm() {
    echo "================================================="
    echo "Start SwapLuncToClsm"
    PARAM_1='{"swap": {"offer_asset": {"info": {"native_token":{"denom":"uluna"}}, "amount": "100000"}}}'
    echo "terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON) "$PARAM_1" --amount 100000uluna $WALLET $TXFLAG"
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON) "$PARAM_1" --amount 100000uluna $WALLET $TXFLAG
    sleep 5
    echo "End"
}

SwapClsmToLunc() {
    echo "================================================="
    echo "Start SwapClsmToLunc"
    MSG='{"swap": {}}'
    ENCODEDMSG=$(echo $MSG | base64 -w 0)
    PARAM_1='{"send": {"contract": "'$(cat $ADDRESS_DIR$CLASSICMOON)'", "amount": "10000000", "msg": "'$ENCODEDMSG'" }}'
    echo "terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG"
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$CLASSICMOON_TOKEN) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
    echo "End"
}

##############################################
######        CLASSICMOON Query         ######
##############################################

GetClassicmoon() {
    echo "================================================="
    echo "GetClassicmoon"
    PARAM_1='{"classicmoon": {}}'
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$CLASSICMOON) "$PARAM_1" $NODECHAIN --output json
    sleep 3
    echo "End"
}

GetPool() {
    echo "================================================="
    echo "GetPool"
    PARAM_1='{"pool": {}}'
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$CLASSICMOON) "$PARAM_1" $NODECHAIN --output json
    sleep 3
    echo "End"
}

SimulationClsmToLunc() {
    echo "================================================="
    echo "SimulationClsmToLunc"
    PARAM_1='{"simulation": {"offer_asset": {"info": {"token":{"contract_addr":"'$(cat $ADDRESS_DIR$CLASSICMOON_TOKEN)'"}}, "amount": "10000000"}}}'
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$CLASSICMOON) "$PARAM_1" $NODECHAIN --output json
    sleep 3
    echo "End"
}

SimulationLuncToClsm() {
    echo "================================================="
    echo "SimulationLuncToClsm"
    PARAM_1='{"simulation": {"offer_asset": {"info": {"native_token":{"denom":"uluna"}}, "amount": "100000"}}}'
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$CLASSICMOON) "$PARAM_1" $NODECHAIN --output json
    sleep 3
    echo "End"
}

ReverseSimulationLuncFromClsm() {
    echo "================================================="
    echo "ReverseSimulationLuncFromClsm"
    PARAM_1='{"reverse_simulation": {"ask_asset": {"info": {"native_token":{"denom":"uluna"}}, "amount": "100000"}}}'
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$CLASSICMOON) "$PARAM_1" $NODECHAIN --output json
    sleep 5
    echo "End"
}

ReverseSimulationClsmFromLunc() {
    echo "================================================="
    echo "ReverseSimulationClsmFromLunc"
    PARAM_1='{"reverse_simulation": {"ask_asset": {"info": {"token":{"contract_addr":"'$(cat $ADDRESS_DIR$CLASSICMOON_TOKEN)'"}}, "amount": "10000000"}}}'
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$CLASSICMOON) "$PARAM_1" $NODECHAIN --output json
    sleep 5
    echo "End"
}

##############################################
#################   Airdrop   ################
##############################################

AirdropGlobalInfo() {
    echo "================================================="
    echo "AirdropGlobalInfo"
    PARAM_1='{"airdrop_global_info": {}}'
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$AIRDROP) "$PARAM_1" $NODECHAIN --output json
    sleep 5
    echo "End"
}

AirdropNftInfo() {
    echo "================================================="
    echo "AirdropNftInfo"
    PARAM_1='{"airdrop_nft_info": {"token_id": "1"}}'
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$AIRDROP) "$PARAM_1" $NODECHAIN --output json
    sleep 5
    echo "End"
}

AirdropUserInfo() {
    echo "================================================="
    echo "AirdropUserInfo"
    NFT_USER="terra1ayy0g44e29z6nsnsdzg7plqv4mv452hzmw24am"
    PARAM_1='{"airdrop_user_info": {"account": "'$NFT_USER'"}}'
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$AIRDROP) "$PARAM_1" $NODECHAIN --output json
    sleep 5
    echo "End"
}

#################################### End of Function ###################################################
BatchUploadAndInstantiate() {
    BatchUpload
    BatchInstantiate
}

BatchConfig() {
    UpdateMinterAsDynamicMinter
    IncreaseAllowanceForAirdrop
    IncreaseAllowanceForClassicMoon
    AddLiquidity
}

BatchTest() {
    Balances

    UpdateMinterAsDynamicMinter

    IncreaseAllowanceForAirdrop
    IncreaseAllowanceForClassicMoon
    GetAllowance
    AddLiquidity
    SwapLuncToClsm
    SwapClsmToLunc
    GetClassicmoon
    GetPool
    SimulationClsmToLunc
    SimulationLuncToClsm
    ReverseSimulationLuncFromClsm
    ReverseSimulationClsmFromLunc
    Balances

    AirdropGlobalInfo
    AirdropNftInfo
    AirdropUserInfo
}

if [[ $FUNCTION == "" ]]; then
    # BatchUploadAndInstantiate
    BatchConfig
else
    $FUNCTION
fi

# 1. Token upload
# 2. Token instantiate
# 3. Replace token address on moon, dynamic, airdrop
# 4. Moon upload and instantiate
# 5. Replace moon address on dynamic
# 6. Dynamic and Airdrop upload and instantiate
# 7. UpdateMinterAsDynamicMinter
# 8. IncreaseAllowanceForAirdrop
# 9. IncreaseAllowanceForClassicMoon
# 10. AddLiquidity
# 