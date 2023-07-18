NETWORK=mainnet
FUNCTION=$1
CATEGORY=$2
PARAM_1=$3
PARAM_2=$4
PARAM_3=$5
ADDR_PRISM="terra1675g95dpcxulmwgyc0hvf66uxn7vcrr5az2vuk"
ADDR_LP="terra1kmcl23z3hrqreua9c78qrqxpjdznh9gj6pxv0eteytypqyrvjc2shm3y4k"

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

SWAP_FACTORY="classicmoon_factory"
SWAP_PAIR="classicmoon_pair"
SWAP_ROUTER="classicmoon_router"
SWAP_TOKEN="classicmoon_token"

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
    #save to FILE_CODE_ID
    echo $CODE_ID > $CODE_DIR$CATEGORY
}

RemoveHistory() {
    rm -rf release
    rm -rf target
    rm -rf info
}

BatchUpload() {
    # CATEGORY=$SWAP_TOKEN
    # printf "y\n" | Upload
    # sleep 3
    
    # CATEGORY=$SWAP_PAIR
    # printf "y\n" | Upload
    # sleep 3

    CATEGORY=$SWAP_FACTORY
    printf "y\n" | Upload
    sleep 3

    CATEGORY=$SWAP_ROUTER
    printf "y\n" | Upload
    sleep 3
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
    # CATEGORY=$SWAP_TOKEN
    # PARAM_1='{"name":"ClassicMoon Test", "symbol":"TCLSM", "decimals":6, "initial_balances":[{"address":"'$ADDR_ADMIN'", "amount":"6800000000000000000"}], "mint":{"minter":"'$ADDR_ADMIN'"}, "marketing":{"marketing":"'$ADDR_ADMIN'","logo":{"url":"https://classicmoon-frontend-2023.web.app/logo83.png"}}}'
    # PARAM_2="TCLSM"
    # printf "y\n" | Instantiate
    # sleep 5

    # CATEGORY=$SWAP_PAIR
    # PARAM_1='{"asset_infos":[{"token":{"contract_addr":"'$(cat $ADDRESS_DIR$SWAP_TOKEN)'"}}, {"native_token":{"denom":"uluna"}}], "token_code_id":'$(cat $CODE_DIR$SWAP_TOKEN)', "asset_decimals":[6, 6]}'
    # PARAM_2="SwapPair"
    # printf "y\n" | Instantiate
    # sleep 5
    
    CATEGORY=$SWAP_FACTORY
    PARAM_1='{"pair_code_id":'$(cat $CODE_DIR$SWAP_PAIR)', "token_code_id":'$(cat $CODE_DIR$SWAP_TOKEN)'}'
    PARAM_2="SwapFactory"
    printf "y\n" | Instantiate
    sleep 5

    CATEGORY=$SWAP_ROUTER
    PARAM_1='{"classicmoon_factory": "'$(cat $ADDRESS_DIR$SWAP_FACTORY)'" }'
    PARAM_2="SwapRouter"
    printf "y\n" | Instantiate
    sleep 5
}

AddNativeTokenDecimal() {
    PARAM_1='{"add_native_token_decimals": {"denom": "uluna", "decimals": 6}}'
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$SWAP_FACTORY) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
    PARAM_1='{"add_native_token_decimals": {"denom": "uusd", "decimals": 6}}'
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$SWAP_FACTORY) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
}

CreatePair() {
    echo "================================================="
    echo "Start Create Pair"
    PARAM_1='{"create_pair": {"assets":[{"info": {"token":{"contract_addr":"'$(cat $ADDRESS_DIR$SWAP_TOKEN)'"}}, "amount": "0"}, {"info": {"native_token":{"denom":"uluna"}}, "amount": "0"}]}}'
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$SWAP_FACTORY) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
    echo "End Create Pair"
}

TokenMint() {
    echo "================================================="
    echo "Mint"
    PARAM_1='{"mint": {"recipient": "terra128a44yv7aa6lr6ee6x8uh9dz80ya4x2kfljqed", "amount": "1000000000000" }}'
    echo "terrad tx wasm execute "terra1p6et9n7nsqa65a9um38g32ugzt5feaat74x2qm" "$PARAM_1" 10uluna $WALLET $TXFLAG"
    printf "y\n" | terrad tx wasm execute "terra1p6et9n7nsqa65a9um38g32ugzt5feaat74x2qm" "$PARAM_1" 10uluna $WALLET $TXFLAG
    sleep 5
}

IncreaseAllowance() {
    echo "================================================="
    echo "Increase Allowance"
    PARAM_1='{"increase_allowance": {"spender": "'$(cat $ADDRESS_DIR$SWAP_PAIR)'", "amount": "100000000000", "expires": {"never": {}}}}'
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$SWAP_TOKEN) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
    echo "End"
}

Allowance() {
    echo "================================================="
    echo "Allowance"
    PARAM_1='{"allowance": {"owner": "'$ADDR_ADMIN'", "spender": "'$(cat $ADDRESS_DIR$SWAP_PAIR)'"}}'
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$SWAP_TOKEN) "$PARAM_1" $NODECHAIN --output json
    sleep 5
    echo "End"
}

##############################################
######                PAIR              ######
##############################################

AddLiquidity() {
    echo "================================================="
    echo "Start Add Liquidity"
    PARAM_1='{"provide_liquidity": {"assets": [{"info": {"token":{"contract_addr":"'$(cat $ADDRESS_DIR$SWAP_TOKEN)'"}}, "amount": "1000000"}, {"info": {"native_token":{"denom":"uluna"}}, "amount": "10000"}]}}'
    echo "terrad tx wasm execute $(cat $ADDRESS_DIR$SWAP_PAIR) "$PARAM_1" --amount 10000uluna $WALLET $TXFLAG"
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$SWAP_PAIR) "$PARAM_1" --amount 10000uluna $WALLET $TXFLAG
    sleep 5
    echo "End"
}

RemoveLiquidity() {
    echo "================================================="
    echo "Start Remove Liquidity"
    MSG='{"withdraw_liquidity": {}}'
    ENCODEDMSG=$(echo $MSG | base64 -w 0)
    echo $ENCODEDMSG
    PARAM_1='{"send": {"contract": "'$(cat $ADDRESS_DIR$SWAP_PAIR)'", "amount": "50000", "msg": "'$ENCODEDMSG'" }}'
    echo "terrad tx wasm execute $ADDR_LP "$PARAM_1" $WALLET $TXFLAG"
    printf "y\n" | terrad tx wasm execute $ADDR_LP "$PARAM_1" $WALLET $TXFLAG
    sleep 5
    echo "End"
}

TokenTransfer () {
    echo "================================================="
    echo "Start Transfer"
    PARAM_1='{"transfer": {"recipient": "terra1tvlszuvjud7ckguglcmzdyh8wx9g0wy5ujhy0h", "amount": "1000000000" }}'
    PARAM_2='TCLSM'
    echo "terrad tx wasm execute terra1cjf9ug5hyq3wate9vlhsdxgvklkv3npcm8u5sfu83gly0c8ljjvq50az2d "$PARAM_1" $WALLET $TXFLAG"
    printf "y\n" | terrad tx wasm execute terra1cjf9ug5hyq3wate9vlhsdxgvklkv3npcm8u5sfu83gly0c8ljjvq50az2d "$PARAM_1" $WALLET $TXFLAG
    sleep 5
    echo "End"
}

TokenBalance() {
    printf "y\n" | terrad query wasm contract-state smart terra10lqax5avsef2ftfvj2ghwhu2elc40e6gxxzxuu2msa67hea3amsqn885ff '{"balance":{"address":"terra10ytakemtdwy3hx5w9rfqdnvyxlhz4tss8zvxrs"}}' $NODECHAIN --output json
}

Balances() {
    echo prism lunc balance
    printf "y\n" | terrad query bank balances $ADDR_PRISM $NODECHAIN --output json
    sleep 3

    echo prism CLSM balance
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$SWAP_TOKEN) '{"balance":{"address":"'$ADDR_PRISM'"}}' $NODECHAIN --output json
    sleep 3

    echo prism LP balance
    printf "y\n" | terrad query wasm contract-state smart $ADDR_LP '{"balance":{"address":"'$ADDR_PRISM'"}}' $NODECHAIN --output json
    sleep 3

    echo pair lunc balance
    printf "y\n" | terrad query bank balances $(cat $ADDRESS_DIR$SWAP_PAIR) $NODECHAIN --output json
    sleep 3

    echo pair CLSM balance
    printf "y\n" | terrad query wasm contract-state smart $(cat $ADDRESS_DIR$SWAP_TOKEN) '{"balance":{"address":"'$(cat $ADDRESS_DIR$SWAP_PAIR)'"}}' $NODECHAIN --output json
    sleep 3
}

NativeBalance() {
    # echo "terrad query bank balances terra1vxq5rfydw89k64k20kt767l5u6wvz3444hpacu $NODECHAIN --output json"
    echo prism account native balance
    printf "y\n" | terrad query bank balances "terra1675g95dpcxulmwgyc0hvf66uxn7vcrr5az2vuk" $NODECHAIN --output json
    sleep 5
    echo prism2 account native balance
    printf "y\n" | terrad query bank balances "terra1tvlszuvjud7ckguglcmzdyh8wx9g0wy5ujhy0h" $NODECHAIN --output json
    sleep 5
    echo pair account native balance
    printf "y\n" | terrad query bank balances "terra1nw42nudu6erp742fx37mm5k3jr3q55gyqu5a9gnegknzf4exwqtqmd2uay" $NODECHAIN --output json
    sleep 5
}

UpdateFactoryConfig() {
    echo "================================================="
    echo "Update Path List for Treasury"
    PARAM_1='{"update_config": {"token_code_id": 7513 }}'
    printf "y\n" | terrad tx wasm execute $(cat $ADDRESS_DIR$SWAP_FACTORY) "$PARAM_1" $WALLET $TXFLAG
    sleep 5
    echo "End"
}

UpdateFactoryOwner() {
    echo "================================================="
    echo "Update Path List for Treasury"
    PARAM_1='{"update_config": {"owner": "terra1pu4jxet2ndtftqmwsrgfsznczrcea3mt6ugcnr" }}'
    printf "y\n" | terrad tx wasm execute terra1pehs2vsd8gadequgmzx36ycrh9hak3f2mxrw20 "$PARAM_1" 10uluna $WALLET $TXFLAG
    sleep 5
    echo "End"
}

UpdateFactoryMigrate() {
    echo "================================================="
    echo "Update Staked Amount"
    PARAM_1='{"migrate_pair": { "contract": "terra100n9lqfn8zm3twamm5ehapn2szy2ljmmnpt0hm", "code_id": 7094 }}'
    printf "y\n" | terrad tx wasm execute terra14pgqzyv8n9ulpcs3jg0ku63cnjs9nc47yc7sjc "$PARAM_1" 10uluna $WALLET $TXFLAG
    sleep 5
    echo "End"
}

#################################### End of Function ###################################################
if [[ $FUNCTION == "" ]]; then
    BatchUpload
    BatchInstantiate
else
    $FUNCTION
fi


##################################################
# 1. Upload
#    - Token 
#    - Pair
#    - Factory
#    - Router
#
# 2. Instantiate
#    - Token
#    - Pair
#    - Factory
#    - Router
#
# 3. AddNativeTokenDecimal (LUNC, USTC)
#    Before this, send LUNC, USTC a bit.
# 4. CreatePair1, CreatePair2, CreatePair3
#
##################################################