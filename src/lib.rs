mod pb;
use pb::{
    raydium_eco_farms::raydium_farm_transaction::Event, raydium_eco_farms::InitializeTransaction,
    raydium_eco_farms::NewRewardTransaction, raydium_eco_farms::RaydiumEcoFarmTransactions,
    raydium_eco_farms::RaydiumFarmTransaction, raydium_eco_farms::RestartOrAddTransaction,
    sf::substreams::solana::v1::Transactions,
};

use sologger_log_context::programs_selector::ProgramsSelector;
use sologger_log_context::sologger_log_context::LogContext;
use substreams::log::println;

const FARM_PROGRAM_ID: &str = "FarmqiPv5eAj3j1GMdMCMUGXqPUvmquZtMy86QH6rzhG";

#[substreams::handlers::map]
fn map_farm_txns(transactions: Transactions) -> Result<Option<RaydiumEcoFarmTransactions>, String> {
    let mut farm_transactions = RaydiumEcoFarmTransactions::default();

    for txn in transactions.transactions.iter() {
        let meta_wrapped = &txn.meta;
        let meta = meta_wrapped.as_ref().unwrap();

        let txn_wrapped = &txn.transaction;
        let transaction = txn_wrapped.as_ref().unwrap();

        let programs_selector: ProgramsSelector = ProgramsSelector::new(&["*".to_string()]);
        let log_contexts = LogContext::parse_logs_basic(&meta.log_messages, &programs_selector);

        let signature = bs58::encode(transaction.signatures.get(0).unwrap()).into_string();

        println(format!("signature: {:?}", signature));

        //get all accounts in base58
        let accounts = transaction
            .message
            .as_ref()
            .unwrap()
            .account_keys
            .iter()
            .map(|account| bs58::encode(account).into_string())
            .collect::<Vec<String>>();

        // println(format!("accounts: {:?}", accounts));
        let initialize_result = process_initialize(&log_contexts, &signature, &accounts);
        if let Ok(Some(initialize_txn)) = initialize_result {
            farm_transactions.transactions.push(RaydiumFarmTransaction {
                event: Some(Event::Initialize(initialize_txn)),
            });
        }
        let restart_or_add_result = process_restart_or_add(&log_contexts, &signature, &accounts);
        if let Ok(Some(restart_or_add_txn)) = restart_or_add_result {
            farm_transactions.transactions.push(RaydiumFarmTransaction {
                event: Some(Event::RestartOrAdd(restart_or_add_txn)),
            });
        }
        let new_reward_result = process_new_reward(&log_contexts, &signature, &accounts);
        if let Ok(Some(new_reward_txn)) = new_reward_result {
            farm_transactions.transactions.push(RaydiumFarmTransaction {
                event: Some(Event::NewReward(new_reward_txn)),
            });
        }
    }
    if farm_transactions.transactions.len() == 0 {
        return Ok(None); // Early return with None
    }

    Ok(Some(farm_transactions))
}

pub fn process_initialize(
    log_contexts: &Vec<LogContext>,
    signature: &String,
    accounts: &Vec<String>,
) -> Result<Option<InitializeTransaction>, String> {
    let process_initialize_logs = log_contexts
        .iter()
        .filter(|context| context.program_id == FARM_PROGRAM_ID)
        .flat_map(|context| {
            context
                .log_messages
                .iter()
                .filter(|log| log.contains("process_initialize reward_per_second"))
                .map(|_| context)
                .collect::<Vec<&LogContext>>()
        })
        .collect::<Vec<&LogContext>>();
    if process_initialize_logs.is_empty() {
        return Ok(None); // Early return with None
    }

    // println(format!(
    //     "process_initialize_logs: {:?}",
    //     process_initialize_logs.get(0)
    // ));
    let user = accounts.get(0);
    let farm_id = accounts.get(1);
    let lp_mint = accounts.get((accounts.len() - 1) - process_initialize_logs.len());
    let reward_mints = accounts
        .iter()
        .skip(accounts.len() - process_initialize_logs.len())
        .collect::<Vec<&String>>();

    println(format!(
        "user: {:?}, farm_id: {:?}, lp_mint: {:?}, reward_mints: {:?}",
        user, farm_id, lp_mint, reward_mints
    ));

    let log_messages = &process_initialize_logs.get(0).unwrap().log_messages;
    // Finding the earliest start time
    let mut start_time: u32 = 0;
    for message in log_messages {
        if let Some(split) = message.split("begin:").nth(1) {
            if let Some(temp_start_time) =
                split.split(",").next().and_then(|s| s.parse::<u32>().ok())
            {
                start_time = if start_time == 0 {
                    temp_start_time
                } else {
                    start_time.min(temp_start_time)
                };
            }
        }
    }
    // "Instruction: Init", "process_initialize accounts len:17", "process_initialize reward_per_second 1653, begin:1737491275, current:1737490622, end:1738096075", "process_initialize reward_per_second 3, begin:1737491287, current:1737490622, end:1738096087"
    println(format!("start_time: {:?}", start_time));

    // Finding the latest end time
    let mut end_time: u32 = 0;
    for message in log_messages {
        if let Some(split) = message.split("end:").nth(1) {
            if let Some(temp_end_time) = split.split(",").next().and_then(|s| s.parse::<u32>().ok())
            {
                end_time = end_time.max(temp_end_time);
            }
        }
    }
    println(format!("end_time: {:?}", end_time));

    Ok(Some(InitializeTransaction {
        signature: signature.to_string(),
        farm_id: farm_id.unwrap().to_string(),
        user: user.unwrap().to_string(),
        lp_mint: lp_mint.unwrap().to_string(),
        reward_mints: reward_mints.iter().map(|x| x.to_string()).collect(),
        start_time,
        end_time,
    }))
}

pub fn process_restart_or_add(
    log_contexts: &Vec<LogContext>,
    signature: &String,
    accounts: &Vec<String>,
) -> Result<Option<RestartOrAddTransaction>, String> {
    let process_restart_logs = log_contexts
        .iter()
        .filter(|context| context.program_id == FARM_PROGRAM_ID)
        .flat_map(|context| {
            context
                .log_messages
                .iter()
                .filter(|log| log.contains("process_creator_restart"))
                .map(|_| context)
                .collect::<Vec<&LogContext>>()
        })
        .collect::<Vec<&LogContext>>();
    let reward_messages = process_restart_logs
        .iter()
        .flat_map(|context| {
            context
                .log_messages
                .iter()
                .filter(|log| log.contains("process_creator_restart"))
        })
        .collect::<Vec<&String>>();
    if reward_messages.is_empty() {
        return Ok(None); // Early return with None
    }
    let user = accounts.get(0);
    let farm_id = accounts.get(1);
    // could get rewards tokens from messages, but are only given the token account not the mint address
    //lp mint token account in accounts, but we need to mint address :-'(
    //no lp mint in the logs

    // println(format!("user: {:?}, farm_id: {:?}", user, farm_id));
    // println(format!("accounts: {:?}", accounts));

    let log_messages = &reward_messages;
    //["process_creator_restart: EVfHjrgu9KFV4889AdyBNtB7jgBhAaPZeSAJ9sY163vD,
    // 1740777211,
    // 1741382011, 16", "process_creator_restart: DpiGX6UpwH7pz9YKka2t6zyWFfBQyiq4ihCy7nzGciEh, 1740777232, 1741382
    // 032, 3"]

    // // Finding the earliest start time
    let mut start_time: u32 = 0;
    for message in log_messages {
        if let Some(split) = message.split("process_creator_restart: ").nth(1) {
            if let Some(temp_start_time) =
                split.split(", ").nth(1).and_then(|s| s.parse::<u32>().ok())
            {
                start_time = if start_time == 0 {
                    temp_start_time
                } else {
                    start_time.min(temp_start_time)
                };
            }
        }
    }

    // // Finding the latest end time
    let mut end_time: u32 = 0;
    for message in log_messages {
        if let Some(split) = message.split("process_creator_restart: ").nth(1) {
            if let Some(temp_end_time) =
                split.split(", ").nth(2).and_then(|s| s.parse::<u32>().ok())
            {
                end_time = end_time.max(temp_end_time);
            }
        }
    }

    Ok(Some(RestartOrAddTransaction {
        signature: signature.to_string(),
        farm_id: farm_id.unwrap().to_string(),
        user: user.unwrap().to_string(),
        start_time,
        end_time,
    }))
}

pub fn process_new_reward(
    log_contexts: &Vec<LogContext>,
    signature: &String,
    accounts: &Vec<String>,
) -> Result<Option<NewRewardTransaction>, String> {
    let process_new_reward_logs = log_contexts
        .iter()
        .filter(|context| context.program_id == FARM_PROGRAM_ID)
        .flat_map(|context| {
            context
                .log_messages
                .iter()
                .filter(|log| log.contains("process_admin_add_reward_token"))
                .map(|_| context)
                .collect::<Vec<&LogContext>>()
        })
        .collect::<Vec<&LogContext>>();
    let reward_messages = process_new_reward_logs
        .iter()
        .flat_map(|context| {
            context
                .log_messages
                .iter()
                .filter(|log| log.contains("process_admin_add_reward_token"))
        })
        .collect::<Vec<&String>>();
    if reward_messages.is_empty() {
        return Ok(None); // Early return with None
    }
    let user = accounts.get(0);
    let farm_id = accounts.get(1);
    // could get rewards tokens from messages, but are only given the token account not the mint address
    //lp mint token account in accounts, but we need to mint address :-'(
    //no lp mint in the logs

    // println(format!("user: {:?}, farm_id: {:?}", user, farm_id));
    // println(format!("accounts: {:?}", accounts));

    let log_messages = &reward_messages;
    //"Program log: process_admin_add_reward_token: 6npFrUXvt7yniYerAwcBjg5SKspxN4tZbGFxEqMFEZHJ, 1740785220, 1741390020, 1, 0

    // // Finding the earliest start time
    let mut start_time: u32 = 0;
    for message in log_messages {
        if let Some(split) = message.split("process_admin_add_reward_token: ").nth(1) {
            if let Some(temp_start_time) =
                split.split(", ").nth(1).and_then(|s| s.parse::<u32>().ok())
            {
                start_time = if start_time == 0 {
                    temp_start_time
                } else {
                    start_time.min(temp_start_time)
                };
            }
        }
    }

    // // Finding the latest end time
    let mut end_time: u32 = 0;
    for message in log_messages {
        if let Some(split) = message.split("process_admin_add_reward_token: ").nth(1) {
            if let Some(temp_end_time) =
                split.split(", ").nth(2).and_then(|s| s.parse::<u32>().ok())
            {
                end_time = end_time.max(temp_end_time);
            }
        }
    }

    Ok(Some(NewRewardTransaction {
        signature: signature.to_string(),
        farm_id: farm_id.unwrap().to_string(),
        user: user.unwrap().to_string(),
        start_time,
        end_time,
    }))
}
