"use strict";

const crypto = require("node:crypto");

/**
 * Escrow settlement layer.
 *
 * ──────────────────────────────────────────────────────────────────────────
 * ⚠️  INTENTIONALLY INCOMPLETE — see README "Known limitations".
 *
 * `settleOnChain` currently SIMULATES the escrow release instead of broadcasting
 * a real transaction. It validates inputs, derives the on-chain jobId exactly as
 * the contract expects (keccak256 of the off-chain job id), and returns a
 * synthetic receipt. It does NOT:
 *   - connect a funded signer (the verifier key) to an RPC endpoint,
 *   - call Escrow.releaseWithVerification(jobId, commitment) on-chain,
 *   - wait for or verify a real transaction receipt.
 *
 * The production implementation is sketched in the comments below using ethers
 * v6. It is left unwired on purpose so the escrow's trusted key is never loaded
 * by accident in this demo. Wiring it is the first real task in the Roadmap.
 * ──────────────────────────────────────────────────────────────────────────
 */

const { keccak256, toUtf8Bytes } = require("ethers");

/** Derive the bytes32 on-chain job id from the off-chain string id. */
function onChainJobId(offChainJobId) {
  return keccak256(toUtf8Bytes(offChainJobId));
}

/**
 * Release escrow to the provider against a verified commitment.
 *
 * @param {object} args
 * @param {string} args.jobId       off-chain job id (string)
 * @param {string} args.commitment  0x-prefixed 32-byte commitment hex OR bare hex
 * @returns {Promise<{simulated: boolean, jobId: string, onChainJobId: string, commitment: string, txHash: string}>}
 */
async function settleOnChain({ jobId, commitment }) {
  if (!jobId) throw new Error("settleOnChain: jobId required");
  if (!commitment) throw new Error("settleOnChain: commitment required");

  const commitment0x = commitment.startsWith("0x") ? commitment : `0x${commitment}`;
  if (!/^0x[0-9a-f]{64}$/.test(commitment0x)) {
    throw new Error("settleOnChain: commitment must be 32-byte lowercase hex");
  }

  const onChainId = onChainJobId(jobId);

  // ── Real implementation (left unwired on purpose) ───────────────────────
  //
  //   const provider = new ethers.JsonRpcProvider(process.env.RPC_URL);
  //   const signer   = new ethers.Wallet(process.env.VERIFIER_PRIVATE_KEY, provider);
  //   const escrow   = new ethers.Contract(process.env.ESCROW_ADDRESS, ESCROW_ABI, signer);
  //   const tx       = await escrow.releaseWithVerification(onChainId, commitment0x);
  //   const receipt  = await tx.wait(1);
  //   return { simulated: false, jobId, onChainJobId: onChainId,
  //            commitment: commitment0x, txHash: receipt.hash };
  //
  // ────────────────────────────────────────────────────────────────────────

  // Simulated receipt: deterministic pseudo-hash so demo flows are testable.
  const fakeTxHash =
    "0x" +
    crypto
      .createHash("sha256")
      .update(`${onChainId}:${commitment0x}`)
      .digest("hex");

  return {
    simulated: true,
    jobId,
    onChainJobId: onChainId,
    commitment: commitment0x,
    txHash: fakeTxHash,
  };
}

module.exports = { settleOnChain, onChainJobId };
