// The agent skill ships inside the CLI, so the instructions an agent reads
// always match the binary it runs.
//
//   pdfvault skill                     print SKILL.md
//
// Installing it into an agent is `npx skills add maxgfr/pdfvault`, which
// knows every agent's skills directory; the CLI does not duplicate that.
import skillMd from "../../../skills/pdfvault/SKILL.md" with { type: "text" };

export const SKILL = skillMd as unknown as string;

/** Where installing the skill into an agent is documented. */
export const INSTALL_HINT = "npx skills add maxgfr/pdfvault";
