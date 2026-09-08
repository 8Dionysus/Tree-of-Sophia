import { describe, expect, it } from "vitest";
import productPage from "../public/apps/aoa-social-connector/index.html?raw";
import privacyPage from "../public/apps/aoa-social-connector/privacy/index.html?raw";
import termsPage from "../public/apps/aoa-social-connector/terms/index.html?raw";

describe("AoA Social Connector public pages", () => {
  it("publishes an accessible product page with visible policy links", () => {
    const page = productPage;

    expect(page).toContain("Dionysus AoA Social Connector");
    expect(page).toContain('href="/apps/aoa-social-connector/privacy/"');
    expect(page).toContain('href="/apps/aoa-social-connector/terms/"');
    expect(page).toContain("official platform APIs");
    expect(page).toContain("explicit human confirmation");
  });

  it("keeps product claims aligned with the owner-only preview", () => {
    const page = productPage;

    expect(page).toContain("limited to the developer's own authorized accounts");
    expect(page).toMatch(/does not\s+scrape social networks/);
    expect(page).not.toContain("automatic publishing");
  });

  it("publishes complete privacy and terms pages without tracking scripts", () => {
    const privacy = privacyPage;
    const terms = termsPage;

    expect(privacy).toContain("Information the connector processes");
    expect(privacy).toContain("Retention and deletion");
    expect(privacy).toContain("does not sell personal data");
    expect(terms).toContain("Human control of publishing");
    expect(terms).toContain("Prohibited use");
    expect(`${privacy}${terms}`).not.toContain("<script");
  });
});
