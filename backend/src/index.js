"use strict";

const express = require("express");
const { verify } = require("./verifier");
const { settleOnChain } = require("./chain");

const app = express();
app.use(express.json({ limit: "1mb" }));

/** Liveness. */
app.get("/health", (_req, res) => res.json({ ok: true }));

/**
 * Verify a submission against a spec. Pure: no settlement, no side effects.
 * Body: { spec: JobSpec, submission: Submission }
 */
app.post("/verify", async (req, res) => {
  const { spec, submission } = req.body || {};
  if (!spec || !submission) {
    return res.status(400).json({ error: "body must contain `spec` and `submission`" });
  }
  try {
    const { verdict } = await verify(spec, submission);
    return res.json({ verdict });
  } catch (e) {
    const status = e.kind === "verification_error" ? 422 : 500;
    return res.status(status).json({ error: e.message });
  }
});

/**
 * Verify and, if the submission passes, settle the escrow to the provider.
 *
 * NOTE: settlement is currently SIMULATED — see chain.js and the README.
 * The response makes this explicit via `settlement.simulated`.
 *
 * Body: { spec: JobSpec, submission: Submission }
 */
app.post("/verify-and-settle", async (req, res) => {
  const { spec, submission } = req.body || {};
  if (!spec || !submission) {
    return res.status(400).json({ error: "body must contain `spec` and `submission`" });
  }
  try {
    const { verdict } = await verify(spec, submission);
    if (!verdict.passed) {
      return res.status(200).json({ verdict, settlement: null, note: "submission did not pass; escrow not released" });
    }
    const settlement = await settleOnChain({ jobId: verdict.job_id, commitment: verdict.commitment });
    return res.json({ verdict, settlement });
  } catch (e) {
    const status = e.kind === "verification_error" ? 422 : 500;
    return res.status(status).json({ error: e.message });
  }
});

const PORT = process.env.PORT || 3000;
if (require.main === module) {
  app.listen(PORT, () => {
    // eslint-disable-next-line no-console
    console.log(`Proof-Gated Escrow backend listening on :${PORT}`);
  });
}

module.exports = app;
