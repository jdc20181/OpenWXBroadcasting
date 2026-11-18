# Forecast and alert posting dashboard (Facebook first)

A small dashboard inside the app that does what the Dubois County Weather site does today,
without needing the website: post weather alerts and the daily forecast to the station's
Facebook Page. Facebook only for now; the design leaves room for other networks as plug-ins.

## Modelled on the DCWeather site (today)

| DCWeather piece | What it does | OpenWX equivalent |
|---|---|---|
| `src/lib/facebook-alerts.js` | Picks alerts by event keywords (warnings, watches, advisories, SWS, MDs, air quality; HWO optional), builds the post text, renders the alert card, posts a photo (`/{page-id}/photos`) or a link post (`/{page-id}/feed`) via the Graph API with `appsecret_proof` | Same rules, run by the app on its own alert feed |
| `src/services/forecast-autopost.js` + `forecast-autopost-settings.js` | Daily forecast post on a schedule (the 7-Day Outlook, today's headline) | Same, using the app's renderer and the station's coverage |
| `sponsorship-settings.js` | Sponsor line / card on posts | Optional sponsor block (station setting) |
| Environment variables (`FACEBOOK_PAGE_ID`, `FACEBOOK_PAGE_ACCESS_TOKEN`, `FACEBOOK_GRAPH_API_VERSION`, app secret) | Server-side credentials | Entered by the user in the dashboard, stored encrypted (below) |
| Imgur upload | Hosting for some images | Not needed: Graph API photo upload takes the PNG directly |

## The dashboard

- **Connect**: Page ID, Page access token (long-lived), optional app secret for
  `appsecret_proof`, Graph API version (defaulted, editable). "Test connection" reads the
  Page name back. Short help text links to Meta's page-token docs.
- **Rules**: which alert types post automatically (the keyword list as checkboxes), which
  only queue for approval, quiet hours, one post per alert ID (updates and cancellations
  reply to or edit the original where the API allows).
- **Forecast schedule**: days and times; which graphic (the default 7-Day Outlook; the
  regional table optional); caption template with station placeholders.
- **Queue**: upcoming and pending posts with a preview of the image and text; Approve,
  Edit, Skip.
- **History**: what was posted, when, the post link, and any error from the API.
- **Pause all**: one switch that stops every automatic post (for test days and outages).

## Credentials and safety

- Stored in the Windows Credential Manager (DPAPI) on the user's PC, never in plain settings
  files, never sent to the OpenWX Relay or any OpenWX server.
- The app posts directly to `graph.facebook.com`; the relay is not involved.
- Practice/test alerts can never post (same guard as the app's practice mode).
- Every automatic post is logged locally; a failed post shows in the dashboard and as a toast.

## Modular shape

`Publisher` trait (connect, test, post image, post text, edit/reply) with a `facebook`
implementation. Other networks are later plug-ins behind the same trait; nothing in the alert
rules or the scheduler knows about Facebook specifically.
