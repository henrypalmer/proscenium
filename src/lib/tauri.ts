import { invoke } from "@tauri-apps/api/core";
import type {
  CatalogSummary,
  ConnectionTestResult,
  Provider,
  ProviderInput,
} from "../types";

export function upsertProvider(provider: ProviderInput): Promise<Provider> {
  return invoke("upsert_provider", { provider });
}

export function listProviders(): Promise<Provider[]> {
  return invoke("list_providers");
}

export function deleteProvider(providerId: string): Promise<void> {
  return invoke("delete_provider", { providerId });
}

export function testProviderConnection(
  provider: ProviderInput,
): Promise<ConnectionTestResult> {
  return invoke("test_provider_connection", { provider });
}

export function getActiveProvider(): Promise<Provider | null> {
  return invoke("get_active_provider");
}

export function setActiveProvider(providerId: string): Promise<void> {
  return invoke("set_active_provider", { providerId });
}

export function refreshCatalog(providerId: string): Promise<void> {
  return invoke("refresh_catalog", { providerId });
}

export function getCatalogSummary(providerId: string): Promise<CatalogSummary> {
  return invoke("get_catalog_summary", { providerId });
}
