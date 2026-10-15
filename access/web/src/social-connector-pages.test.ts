import { describe, expect, it } from "vitest";
import productPage from "../public/apps/aoa-social-connector/index.html?raw";
import privacyPage from "../public/apps/aoa-social-connector/privacy/index.html?raw";
import termsPage from "../public/apps/aoa-social-connector/terms/index.html?raw";

describe("AoA Social Connector public pages", () => {
  it("links the product page to its policy pages", () => {
    expect(productPage).toContain('href="/apps/aoa-social-connector/privacy/"');
    expect(productPage).toContain('href="/apps/aoa-social-connector/terms/"');
  });

  it("serves privacy and terms without scripts", () => {
    expect(`${privacyPage}${termsPage}`).not.toContain("<script");
  });
});
