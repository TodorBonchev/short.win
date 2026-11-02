#!/usr/bin/env node
/** One bounded operator pass. Use an external supervisor/timer; dry-run is default. */
import { address } from "@solana/kit";
import {
  runOperator,
  stringify,
  type OperatorOptions,
} from "../src/operator.ts";
if (process.argv.includes('--help')) {
 console.log(`One bounded operator pass; dry-run is the default.
Required: --settings FILE --database FILE --service-database FILE --evidence-directory DIR
RPC: --rpc-url URL or SOLANA_RPC_URL
Optional: --tick --keeper-address ADDRESS --reporter-keyfile FILE --keeper-keyfile FILE
--execute explicitly signs/submits; never enable until roles, funding and settings are reviewed.
Bounds: --max-accounts N --max-event-transactions N --max-actions N --max-fee-lamports N
--max-pass-fees-lamports N --min-balance-lamports N --max-temporary-lamports N --max-reference-rent-lamports N --max-new-reference-accounts N --max-route-setup-lamports N --max-daily-publication-fees-lamports N
Run passes with a supervisor that prevents overlap. The HTTP bridge does not sign.`);
 process.exit(0);
}
const args = process.argv.slice(2);
const values = new Map<string, string>();
const flags = new Set<string>();
const boolean = new Set(["execute", "tick"]);
const permitted = new Set([
  "rpc-url",
  "database",
  "service-database",
  "settings",
  "evidence-directory",
  "keeper-address",
  "reporter-keyfile",
  "keeper-keyfile",
  "python",
  "max-accounts",
  "max-event-transactions",
  "max-actions",
  "max-fee-lamports",
  "max-pass-fees-lamports",
  "min-balance-lamports",
  "max-temporary-lamports",
  "max-reference-rent-lamports",
  "max-new-reference-accounts",
  "max-route-setup-lamports",
  "max-daily-publication-fees-lamports",
]);
for (let i = 0; i < args.length; i++) {
  const arg = args[i]!;
  if (!arg.startsWith("--")) throw new Error("Expected --named options");
  const key = arg.slice(2);
  if (boolean.has(key)) {
    flags.add(key);
    continue;
  }
  if (!permitted.has(key) || values.has(key))
    throw new Error("Unknown/duplicate option " + arg);
  const value = args[++i];
  if (!value || value.startsWith("--")) throw new Error("Missing " + arg);
  values.set(key, value);
}
function required(key: string): string {
  const value = values.get(key);
  if (!value) throw new Error("Required --" + key);
  return value;
}
function count(key: string): number | undefined {
  const v = values.get(key);
  if (v === undefined) return undefined;
  if (!/^[1-9][0-9]*$/.test(v)) throw new Error("Invalid count " + key);
  return Number(v);
}
function amount(key: string): bigint | undefined {
  const v = values.get(key);
  if (v === undefined) return undefined;
  if (!/^(0|[1-9][0-9]*)$/.test(v)) throw new Error("Invalid amount " + key);
  return BigInt(v);
}
const rpcUrl = values.get("rpc-url") ?? process.env.SOLANA_RPC_URL;
if (!rpcUrl) throw new Error("Set SOLANA_RPC_URL or --rpc-url");
const options: OperatorOptions = {
  rpcUrl,
  database: required("database"),
  serviceDatabase: required("service-database"),
  serviceSettings: required("settings"),
  evidenceDirectory: required("evidence-directory"),
  keeperAddress: values.has("keeper-address")
    ? address(values.get("keeper-address")!)
    : undefined,
  reporterKeyfile: values.get("reporter-keyfile"),
  keeperKeyfile: values.get("keeper-keyfile"),
  python: values.get("python"),
  execute: flags.has("execute"),
  tick: flags.has("tick"),
  maxAccounts: count("max-accounts"),
  maxEventTransactions: count("max-event-transactions"),
  maxActions: count("max-actions"),
  maxFeeLamports: amount("max-fee-lamports"),
  maxPassFeesLamports: amount("max-pass-fees-lamports"),
  minBalanceLamports: amount("min-balance-lamports"),
  maxTemporaryLamports: amount("max-temporary-lamports"),
  maxReferenceRentLamports: amount("max-reference-rent-lamports"),
  maxNewReferenceAccounts: count("max-new-reference-accounts"),
  maxRouteSetupLamports: amount("max-route-setup-lamports"),
  maxDailyPublicationFeesLamports: amount("max-daily-publication-fees-lamports"),
};
const result = await runOperator(options);
process.stdout.write(stringify(result) + "\n");
if (result.alerts.length) process.exitCode = 2;
