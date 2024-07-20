import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SolanaRpgEngine, IDL } from "../target/types/solana_rpg_engine";
import { assert } from "chai";
import NodeWallet from "@coral-xyz/anchor/dist/cjs/nodewallet";

describe("solana-rpg-engine", () => {
  //configure the client to use the local cluster
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.SolanaRpgEngine as Program<SolanaRpgEngine>;
  const wallet = anchor.workspace.SolanaRpgEngine.provider.wallet.payer as anchor.web3.Keypair;

  //create a game master and player
  const gameMaster = wallet;
  const player = wallet;

  //create the game treasury to deposit lamports
  const treasury = anchor.web3.Keypair.generate();

  //writing all our tests
  it("Create Game", async () => {
    //derive the game address
    const [gameKey] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("GAME"), treasury.publicKey.toBuffer()],
      program.programId
    );

    //use the create game instruction
    const tx = await program.methods
    .createGame(8,
      //8 items per player
    )
    .accounts({
      game: gameKey,
      gameMaster : gameMaster.publicKey,
      treasury : treasury.publicKey,
      systemProgram : anchor.web3.SystemProgram.programId
    })
    .signers([treasury])
    .rpc();

    //confirm tx
    await program.provider.connection.confirmTransaction(tx);
  });

  it("Create Player", async () => {
    const [gameKey] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("GAME"), treasury.publicKey.toBuffer()],
      program.programId
    );

    //deriving the player account address for this particular game
    const [playerKey] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("PLAYER"), gameKey.toBuffer(), player.publicKey.toBuffer()],
      program.programId
    );

    //create player instruction
    const txHash = await program.methods
      .createPlayer()
      .accounts({
        game: gameKey,
        playerAccount: playerKey,
        player: player.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    await program.provider.connection.confirmTransaction(txHash);
  });

  it("Spawn Monster", async () => {
    const [gameKey] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("GAME"), treasury.publicKey.toBuffer()],
      program.programId
    );

    //get the player account PDA
    const [playerKey] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("PLAYER"), gameKey.toBuffer(), player.publicKey.toBuffer()],
      program.programId
    );

    const playerAccount = await program.account.player.fetch(playerKey);

    //derive the spawned monsted
    const [monsterKey] = anchor.web3.PublicKey.findProgramAddressSync([
      Buffer.from("MONSTER"),gameKey.toBuffer(),player.publicKey.toBuffer(), playerAccount.nextMonsterIndex.toBuffer('le',8)
    ],program.programId);

    //spawn monster method transaction
    const tx = await program.methods
    .spawnMonster()
    .accounts({
      game: gameKey,
      playerAccount: playerKey,
      monster: monsterKey,
      player: player.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();

    await program.provider.connection.confirmTransaction(tx);
  });

  it("Attack Monster", async () => {
    //re-deriving the player account, game and monster PDAs
    const [gameKey] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("GAME"), treasury.publicKey.toBuffer()],
      program.programId
    );

    const [playerKey] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("PLAYER"), gameKey.toBuffer(), player.publicKey.toBuffer()],
      program.programId
    );
      
    // Fetch the latest monster created
    const playerAccount = await program.account.player.fetch(playerKey);
    const [monsterKey] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("MONSTER"), gameKey.toBuffer(), player.publicKey.toBuffer(), playerAccount.nextMonsterIndex.subn(1).toBuffer('le', 8)],
      program.programId
    );

    const tx = await program.methods
    .attackMonster()
    .accounts({
      playerAccount: playerKey,
      monster: monsterKey,
      player: player.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();

    await program.provider.connection.confirmTransaction(tx);

    //get the monster account and check if hitpoints reduced by 1 after the attack
    const monsterAccount = await program.account.monster.fetch(monsterKey);
    assert(monsterAccount.hitpoints.eqn(99));
  });

  it("Deposit Action Points", async () => {
    //deriving the game and player account
    const [gameKey] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("GAME"), treasury.publicKey.toBuffer()],
      program.programId
    );

    const [playerKey] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("PLAYER"), gameKey.toBuffer(), player.publicKey.toBuffer()],
      program.programId
    );

    //lets give it to a clockwork bot -> to show that anyone can deposit
    const clockworkWallet = anchor.web3.Keypair.generate();

    //to give it a starting balance
    const clockworkProvider = new anchor.AnchorProvider(
      program.provider.connection,
      new NodeWallet(clockworkWallet),
      anchor.AnchorProvider.defaultOptions()
    );

    //creating an instance of clockwork program -> dont worry if you dont get it instantly
    const clockworkProgram = new anchor.Program<SolanaRpgEngine>(
      IDL,
      program.programId,
      clockworkProvider
    );

    //giving the clockwork wallet and the treasury some lamports
    const amountToInitialize = 10000000000;

    const clockworkAirdropTx = await clockworkProgram.provider.connection.requestAirdrop(clockworkWallet.publicKey, amountToInitialize);
    await program.provider.connection.confirmTransaction(clockworkAirdropTx, "confirmed");

    const treasuryAirdropTx = await clockworkProgram.provider.connection.requestAirdrop(treasury.publicKey, amountToInitialize);
    await program.provider.connection.confirmTransaction(treasuryAirdropTx, "confirmed");

    //calling the method to deposit the lamports
    const tx = await program.methods
    .depositActionPoints()
    .accounts({
      game: gameKey,
      player: playerKey,
      treasury: treasury.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();

    await program.provider.connection.confirmTransaction(tx);

    //player create + monster spawn + attack monster -> these many are due to the treasury
    const expectedActionPoints = 100 + 5 + 1; 

    const treasuryBalance = await program.provider.connection.getBalance(treasury.publicKey);
    assert(
      treasuryBalance == 
      (amountToInitialize + expectedActionPoints) // Player Create ( 100 ) + Monster Spawn ( 5 ) + Monster Attack ( 1 )
    );

    const gameAccount = await program.account.game.fetch(gameKey);
    assert(gameAccount.actionPointsCollected.eqn(expectedActionPoints));

    const playerAccount = await program.account.player.fetch(playerKey);
    assert(playerAccount.actionPointsSpent.eqn(expectedActionPoints));

    //once deposited into treasury -> to be collected becomes 0
    console.log("✅Transaction was successful.");
    assert(playerAccount.actionPointsToBeCollected.eqn(0));
  });

});
