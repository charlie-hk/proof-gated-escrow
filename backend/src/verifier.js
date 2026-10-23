"use strict";

const { execFile } = require("node:child_process");
const { mkdtemp, writeFile, rm } = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");

/**
 * Absolute path to the compiled Rust verifier binary. Override with
 * VERIFIER_BIN when the release binary lives elsewhere (e.g. in a container).
 */
const VERIFIER_BIN =
  process.env.VERIFIER_BIN ||
  path.resolve(__dirname, "../../rust-verifier/target/release/verifier-cli");

/**
 * Run the deterministic Rust verifier over a spec and submission.
 *
 * We write both inputs to a short-lived temp dir and pass their paths, rather
 * than streaming, so the call is simple and the CLI's two-path contract is used
 * exactly as designed. The temp dir is always removed.
 *
 * CLI contract (rust-verifier/src/bin/verifier_cli.rs):
 *   exit 0 -> passed,  stdout = Verdict JSON
 *   exit 2 -> failed,  stdout = Verdict JSON
 *   exit 3 -> error,   stderr = { "error": "..." }
 *
 * @param {object} spec        JobSpec object
 * @param {object} submission  Submission object
 * @returns {Promise<{verdict: object}>}
 */
async function verify(spec, submission) {
  const dir = await mkdtemp(path.join(os.tmpdir(), "nbv-"));
  const specPath = path.join(dir, "spec.json");
  const subPath = path.join(dir, "submission.json");
  try {
    await writeFile(specPath, JSON.stringify(spec));
    await writeFile(subPath, JSON.stringify(submission));
    return await runCli(specPath, subPath);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

function runCli(specPath, subPath) {
  return new Promise((resolve, reject) => {
    execFile(
      VERIFIER_BIN,
      [specPath, subPath],
      { timeout: 10_000, maxBuffer: 4 * 1024 * 1024 },
      (err, stdout, stderr) => {
        const code = err && typeof err.code === "number" ? err.code : 0;

        if (code === 0 || code === 2) {
          try {
            return resolve({ verdict: JSON.parse(stdout) });
          } catch (e) {
            return reject(new Error(`verifier produced unparseable output: ${e.message}`));
          }
        }

        if (code === 3) {
          let message = "verification error";
          try {
            message = JSON.parse(stderr).error || message;
          } catch (_) {
            /* keep default */
          }
          return reject(Object.assign(new Error(message), { kind: "verification_error" }));
        }

        return reject(
          new Error(`verifier failed (code ${code}): ${stderr || (err && err.message) || "unknown"}`)
        );
      }
    );
  });
}

module.exports = { verify, VERIFIER_BIN };
