import type { AppSummary } from "@/types/app";

/**
 * Mirrors the routing logic in src-tauri/src/commands/execute.rs `route_and_execute()`.
 * Returns true when the update will be delegated (opens the app's own updater)
 * rather than handled directly (brew upgrade, direct download, etc.).
 */
export function isDelegatedUpdate(app: AppSummary): boolean {
  // Phase 1: route by updateSource (available_update.source_type)
  if (app.updateSource) {
    switch (app.updateSource) {
      case "adobe_cc":
        return true;

      case "mas":
        // MAS executor handles it directly
        return false;

      case "sparkle":
        // Sparkle usually has a download URL — assume direct
        return false;

      case "homebrew_cask":
      case "github":
      case "homebrew_api":
        // Direct if we have a cask token (can fall back to brew CLI)
        if (app.homebrewCaskToken) return false;
        // No cask token — falls through to Phase 2
        break;

      // keystone, microsoft_autoupdate, jetbrains_toolbox, electron, mozilla, etc.
      // All fall through to Phase 2
      default:
        break;
    }
  }

  // Phase 2: route by installSource
  if (app.installSource === "homebrew_formula" && app.homebrewFormulaName) {
    return false;
  }
  if (app.installSource === "homebrew" && app.homebrewCaskToken) {
    return false;
  }
  if (app.installSource === "mac_app_store") {
    return false;
  }

  // Everything else → delegated (DelegatedExecutor opens the app)
  return true;
}
