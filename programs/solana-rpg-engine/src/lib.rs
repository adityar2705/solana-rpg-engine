use anchor_lang::prelude::*;
use anchor_lang::system_program::{Transfer, transfer};
use anchor_lang::solana_program::log::sol_log_compute_units;

declare_id!("GdjgmDu1hZKddGPc1YdsR5Y2ZWQAJNXJywXhk7WXbbQk");

// ----------- ACCOUNTS ----------
#[account]
//game struct with game_master and treasury
pub struct Game{
    pub game_master : Pubkey, 
    pub treasury : Pubkey,

    pub action_points_collected : u64,
    pub game_config : GameConfig
}

#[account]
//every game-player combination will be a new account
pub struct  Player{
    pub player : Pubkey,
    pub game : Pubkey,
    pub action_points_spent : u64,
    pub action_points_to_be_collected: u64, 

    pub status_flag: u8,                
    pub experience: u64,   
    pub kills : u64,
    pub next_monster_index : u64,

    //saving some space for future use
    pub for_future_use : [u8;256],

    //player inventory
    pub inventory : Vec<InventoryItem>
}

#[account]
//monster data struct
pub struct Monster {
    pub player: Pubkey, 
    pub game: Pubkey,

    //points for killing the monster
    pub hitpoints: u64,
}

// ----------- GAME CONFIG ----------
//this is how we create types for our own personal use in Anchor
#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct GameConfig{
    pub max_items_per_player : u8,
    pub for_future_use : [u64;16],
}

// ----------- STATUS ----------
//creating the status flags -> using bit operations
const IS_FROZEN_FLAG : u8 = 1<<0;
const IS_POISONED : u8 = 1<<1;
const IS_BURNING_FLAG: u8 = 1 << 2;
const IS_BLESSED_FLAG: u8 = 1 << 3;
const IS_CURSED_FLAG: u8 = 1 << 4;
const IS_STUNNED_FLAG: u8 = 1 << 5;
const IS_SLOWED_FLAG: u8 = 1 << 6;
const IS_BLEEDING_FLAG: u8 = 1 << 7;
const NO_EFFECT_FLAG : u8 = 0b00000000;

// ----------- INVENTORY ----------
#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct InventoryItem{
    pub name : [u8;32], //32 bytes for the name of the item -> fixed
    pub amount : u64,
    pub for_future_use: [u8; 128],
}

// ----------- HELPER ----------
//players will send action points (lamports) to the game treasury as payment for in-game actions
pub fn spend_action_points<'info>(
    action_points : u64,

    //of the Player PDA type (custom for the game)
    player_account : &mut Account<'info,Player>,

    //we will have to pass in the player and system program (player.to_account_info().clone())
    player : &AccountInfo<'info>,
    system_program : &AccountInfo<'info>
) -> Result<()>{

    //add the necessary points spent to the player's account
    player_account.action_points_spent = player_account.action_points_spent.checked_add(action_points).unwrap();
    player_account.action_points_to_be_collected = player_account.action_points_to_be_collected.checked_add(action_points).unwrap();

    //create a CPI instruction
    let cpi_context = CpiContext::new(
        //the program we are using
        system_program.clone(),

        //creating the instruction
        Transfer{
            from:player.clone(),
            to : player_account.to_account_info().clone()
        }
    );

    //transfer lamports from player's public key to the Player account (custom for the game) -> transfer to treasury later
    transfer(cpi_context, action_points)?;

    msg!("Minus {} action points", action_points);
    Ok(())
}

// ----------- CREATE GAME ----------
#[derive(Accounts)]
pub struct CreateGame<'info>{
    #[account(
        init,
        seeds = [b"GAME",treasury.key().as_ref()],
        bump,
        payer = game_master,

        //initialize with the size of the Game struct + 8 -> anchor discriminator
        space = std::mem::size_of::<Game>() + 8
    )]
    pub game : Account<'info,Game>,

    //the owner of the game
    #[account(mut)]
    pub game_master: Signer<'info>,

    /// CHECK: Need to know they own the treasury
    pub treasury: Signer<'info>,
    pub system_program : Program<'info,System>
}

//function to run the create game logic
pub fn run_create_game(ctx : Context<CreateGame>, max_items_per_player : u8)->Result<()>{
    ctx.accounts.game.game_master = ctx.accounts.game_master.key().clone();
    ctx.accounts.game.treasury = ctx.accounts.treasury.key().clone();

    ctx.accounts.game.action_points_collected = 0;
    ctx.accounts.game.game_config.max_items_per_player = max_items_per_player;

    msg!("Game created!");
    Ok(())
}

// ----------- CREATE PLAYER ----------
#[derive(Accounts)]
//struct for the create player function
pub struct CreatePlayer<'info>{
    //to use storage of Heap
    pub game : Box<Account<'info,Game>>,

    //creating the player account PDA (custom for the game)
    #[account(
        init, 
        seeds=[
            b"PLAYER", 
            game.key().as_ref(), 
            player.key().as_ref()
        ], 
        bump, 
        payer = player, 
        space = std::mem::size_of::<Player>() + std::mem::size_of::<InventoryItem>() * game.game_config.max_items_per_player as usize + 8)
    ]
    pub player_account: Account<'info,Player>,

    #[account(mut)]
    pub player: Signer<'info>,

    pub system_program: Program<'info, System>,
}

//create player function
pub fn run_create_player(ctx : Context<CreatePlayer>)-> Result<()>{
    ctx.accounts.player_account.player = ctx.accounts.player.key().clone();
    ctx.accounts.player_account.game = ctx.accounts.game.key().clone();

    ctx.accounts.player_account.status_flag = NO_EFFECT_FLAG;
    ctx.accounts.player_account.experience = 0;
    ctx.accounts.player_account.kills = 0;

    msg!("Hero has entered the game!");

    //we are spending 100 action points(lamports) to create the player
    {
        spend_action_points(
            100,
            &mut ctx.accounts.player_account,

            //we need to pass in the info of these
            &ctx.accounts.player.to_account_info(),
            &ctx.accounts.system_program.to_account_info()
        )?;
    }
    Ok(())
}

// ----------- SPAWN MONSTER ----------
#[derive(Accounts)]
pub struct CreateMonster<'info>{
    pub game : Box<Account<'info,Game>>,

    //the game and player defined in the struct should be the one in the Player PDA  
    #[account(mut, has_one = game, has_one = player)]
    pub player_account : Box<Account<'info,Player>>,

    #[account(
        init,
        seeds = [
            b"MONSTER",
            game.key().as_ref(),
            player.key().as_ref(),
            player_account.next_monster_index.to_le_bytes().as_ref()
        ],
        bump,
        payer = player,
        space = std::mem::size_of::<Monster>() + 8
    )]
    pub monster : Account<'info,Monster>,

    //the player should be a signer
    #[account(mut)]
    pub player: Signer<'info>,

    pub system_program: Program<'info, System>,
}

//function for the player to spawn a monster
pub fn run_spawn_monster(ctx : Context<CreateMonster>) ->Result<()>{
    ctx.accounts.monster.player = ctx.accounts.player.key();
    ctx.accounts.monster.game = ctx.accounts.game.key().clone();
    ctx.accounts.monster.hitpoints = 100;

    msg!("Monster Spawned!");

    ctx.accounts.player_account.next_monster_index = ctx.accounts.player_account.next_monster_index.checked_add(1).unwrap();

    //spend 5 action points to spawn the monster
    {
        spend_action_points(5, &mut ctx.accounts.player_account, &ctx.accounts.player.to_account_info(), &ctx.accounts.system_program.to_account_info())?;
    } 
    Ok(())
}

// ----------- ATTACK MONSTER ----------
#[derive(Accounts)]
pub struct AttackMonster<'info>{
    #[account(mut, has_one = player)]
    pub player_account : Box<Account<'info, Player>>,

    #[account(mut, has_one = player, constraint = monster.game == player_account.game)]
    //make sure they are both referencing the same game
    pub monster: Box<Account<'info, Monster>>,

    #[account(mut)]
    pub player: Signer<'info>,

    //necessary account
    pub system_program: Program<'info, System>,
}

//function to attack the monster
pub fn run_attack_monster(ctx : Context<AttackMonster>) ->Result<()>{
    let mut did_kill = false;
    {
        let hp_before_attack = ctx.accounts.monster.hitpoints;

        //saturating_sub to avoid overflow
        let hp_after_attack = ctx.accounts.monster.hitpoints.saturating_sub(1);
        let damage_dealt = hp_before_attack - hp_after_attack;

        ctx.accounts.monster.hitpoints = hp_after_attack;

        //check whether monster has died or not
        if hp_before_attack > 0 && hp_after_attack == 0{
            did_kill = true;
        }

        if damage_dealt > 0{
            msg!("Damage Dealt: {}", damage_dealt);
        }else{
            msg!("Stop it's already dead!");
        }
    }

    {
        ctx.accounts.player_account.experience = ctx.accounts.player_account.experience.saturating_add(1);
        msg!("+1 EXP!");

        //add 1 to the kills -> using safe math (saturating_add)
        if did_kill {
            ctx.accounts.player_account.kills = ctx.accounts.player_account.kills.saturating_add(1);
            msg!("You killed the monster!");
        }
    }

    {
        //spend 1 action point(lamport) to kill the monster
        spend_action_points(
            1, 
            &mut ctx.accounts.player_account,
            &ctx.accounts.player.to_account_info(), 
            &ctx.accounts.system_program.to_account_info()
        )?;
    }

    Ok(())
}

// ----------- REDEEM TO TREASURY ----------
#[derive(Accounts)]
pub struct CollectActionPoints<'info>{
    //has_one -> the treasury of this game must be the treasury of this account
    #[account(mut, has_one = treasury)]
    pub game : Box<Account<'info,Game>>,

    #[account(
        mut,
        has_one=game
    )]
    //player PDA account
    pub player: Box<Account<'info, Player>>,

    #[account(mut)]
    /// CHECK: It's being checked in the game account
    pub treasury: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

//anyone who pays for the Tx fee can run this instruction
pub fn run_collect_action_points(ctx : Context<CollectActionPoints>) -> Result<()>{
    let transfer_amount : u64 = ctx.accounts.player.action_points_to_be_collected;

    //transfer the action points -> from player account( although mentioned as player) to treasury
    **ctx.accounts.player.to_account_info().try_borrow_mut_lamports()? -= transfer_amount;
    **ctx.accounts.treasury.to_account_info().try_borrow_mut_lamports()? += transfer_amount;

    ctx.accounts.player.action_points_to_be_collected = 0;

    //how many action points the game has collected
    ctx.accounts.game.action_points_collected = ctx.accounts.game.action_points_collected.checked_add(transfer_amount).unwrap();
    msg!("The treasury collected {} action points to treasury", transfer_amount);

    Ok(())
}

#[program]
pub mod solana_rpg_engine {
    use super::*;

    //instruction to create the game
    pub fn create_game(ctx: Context<CreateGame>, max_items_per_player: u8) -> Result<()> {
        run_create_game(ctx, max_items_per_player)?;
        sol_log_compute_units();
        Ok(())
    }

    //instruction to create the player
    pub fn create_player(ctx: Context<CreatePlayer>) -> Result<()> {
        run_create_player(ctx)?;
        sol_log_compute_units();
        Ok(())
    }

    ////instruction to spawn the monster
    pub fn spawn_monster(ctx: Context<CreateMonster>) -> Result<()> {
        run_spawn_monster(ctx)?;
        sol_log_compute_units();
        Ok(())
    }

    //instruction to attack the monster
    pub fn attack_monster(ctx: Context<AttackMonster>) -> Result<()> {
        run_attack_monster(ctx)?;
        sol_log_compute_units();
        Ok(())
    }

    //instruction to deposit the attack points to the in-game treasury
    pub fn deposit_action_points(ctx: Context<CollectActionPoints>) -> Result<()> {
        run_collect_action_points(ctx)?;
        sol_log_compute_units();
        Ok(())
    }
}
