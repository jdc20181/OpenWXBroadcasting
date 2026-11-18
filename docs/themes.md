# Themes

Two separate things, kept separate:

1. **App theme**: the studio's own interface (panels, ribbon, dialogs).
2. **On-air look**: the graphics and map overlays viewers see (station brand applied on top).

## App themes (built in)

| Theme | For |
|---|---|
| **Dark** (default) | Studios and control rooms; OpenWX navy `#1B232E` base, orange `#EE7D14` accent |
| **Light** | Daytime offices, bright rooms |
| **High contrast** | Accessibility (Windows high-contrast aware) |

## On-air looks (built in)

| Look | Character |
|---|---|
| **Broadcast Crisp / Modern** | Today's CrowdedWX Modern look: angled plates, gloss, Meteocons icons |
| **Classic** | Today's Classic look: flat plates, drawn icons |
| **Light** | Light panels for daytime and print/social use |
| **Dark** | Deep panels for night shows |

Each look is a token set (colours, plate shape, fonts, icon set, animation style), not code, so
stations can duplicate and edit one, and new looks can ship without an app update. The
station's brand colours, logo and fonts are applied on top of whichever look is chosen.

## Carry-over from CrowdedWX

- The look system already restyles by layer name and is reversible (Modern ↔ Classic);
  Light and Dark are additions to the same system.
- The CrowdedWX cyan stays CrowdedWX-only; OpenWX's accent is the logo orange.
- Map themes (Map Studio > Map themes) stay as they are and appear as a third, map-only list.
