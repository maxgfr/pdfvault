//! Glyph names and base encodings for simple fonts, generated from the
//! Adobe Glyph List and the PDF base encodings (as carried by lopdf), kept
//! as compact tables: a sorted name list searched by bisection.

/// Code → Unicode (0 = no glyph).
pub(crate) const STANDARD: [u16; 256] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0020, 0x0021, 0x0022, 0x0023,
    0x0024, 0x0025, 0x0026, 0x2019, 0x0028, 0x0029, 0x002A, 0x002B, 0x002C, 0x002D, 0x002E, 0x002F,
    0x0030, 0x0031, 0x0032, 0x0033, 0x0034, 0x0035, 0x0036, 0x0037, 0x0038, 0x0039, 0x003A, 0x003B,
    0x003C, 0x003D, 0x003E, 0x003F, 0x0040, 0x0041, 0x0042, 0x0043, 0x0044, 0x0045, 0x0046, 0x0047,
    0x0048, 0x0049, 0x004A, 0x004B, 0x004C, 0x004D, 0x004E, 0x004F, 0x0050, 0x0051, 0x0052, 0x0053,
    0x0054, 0x0055, 0x0056, 0x0057, 0x0058, 0x0059, 0x005A, 0x005B, 0x005C, 0x005D, 0x005E, 0x005F,
    0x2018, 0x0061, 0x0062, 0x0063, 0x0064, 0x0065, 0x0066, 0x0067, 0x0068, 0x0069, 0x006A, 0x006B,
    0x006C, 0x006D, 0x006E, 0x006F, 0x0070, 0x0071, 0x0072, 0x0073, 0x0074, 0x0075, 0x0076, 0x0077,
    0x0078, 0x0079, 0x007A, 0x007B, 0x007C, 0x007D, 0x007E, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x00A1, 0x00A2, 0x00A3, 0x2044, 0x00A5, 0x0192, 0x00A7,
    0x00A4, 0x0027, 0x201C, 0x00AB, 0x2039, 0x203A, 0xFB01, 0xFB02, 0x0000, 0x2013, 0x2020, 0x2021,
    0x00B7, 0x0000, 0x00B6, 0x2022, 0x201A, 0x201E, 0x201D, 0x00BB, 0x2026, 0x2030, 0x0000, 0x00BF,
    0x0000, 0x0060, 0x00B4, 0x02C6, 0x02DC, 0x00AF, 0x02D8, 0x02D9, 0x00A8, 0x0000, 0x02DA, 0x00B8,
    0x0000, 0x02DD, 0x02DB, 0x02C7, 0x2014, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x00C6, 0x0000, 0x00AA,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0141, 0x00D8, 0x0152, 0x00BA, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x00E6, 0x0000, 0x0000, 0x0000, 0x0131, 0x0000, 0x0000, 0x0142, 0x00F8, 0x0153, 0x00DF,
    0x0000, 0x0000, 0x0000, 0x0000,
];

/// Code → Unicode (0 = no glyph).
pub(crate) const WIN_ANSI: [u16; 256] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0020, 0x0021, 0x0022, 0x0023,
    0x0024, 0x0025, 0x0026, 0x0027, 0x0028, 0x0029, 0x002A, 0x002B, 0x002C, 0x002D, 0x002E, 0x002F,
    0x0030, 0x0031, 0x0032, 0x0033, 0x0034, 0x0035, 0x0036, 0x0037, 0x0038, 0x0039, 0x003A, 0x003B,
    0x003C, 0x003D, 0x003E, 0x003F, 0x0040, 0x0041, 0x0042, 0x0043, 0x0044, 0x0045, 0x0046, 0x0047,
    0x0048, 0x0049, 0x004A, 0x004B, 0x004C, 0x004D, 0x004E, 0x004F, 0x0050, 0x0051, 0x0052, 0x0053,
    0x0054, 0x0055, 0x0056, 0x0057, 0x0058, 0x0059, 0x005A, 0x005B, 0x005C, 0x005D, 0x005E, 0x005F,
    0x0060, 0x0061, 0x0062, 0x0063, 0x0064, 0x0065, 0x0066, 0x0067, 0x0068, 0x0069, 0x006A, 0x006B,
    0x006C, 0x006D, 0x006E, 0x006F, 0x0070, 0x0071, 0x0072, 0x0073, 0x0074, 0x0075, 0x0076, 0x0077,
    0x0078, 0x0079, 0x007A, 0x007B, 0x007C, 0x007D, 0x007E, 0x2022, 0x20AC, 0x2022, 0x201A, 0x0192,
    0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x2022, 0x017D, 0x2022,
    0x2022, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A,
    0x0153, 0x2022, 0x017E, 0x0178, 0x0020, 0x00A1, 0x00A2, 0x00A3, 0x00A4, 0x00A5, 0x00A6, 0x00A7,
    0x00A8, 0x00A9, 0x00AA, 0x00AB, 0x00AC, 0x002D, 0x00AE, 0x00AF, 0x00B0, 0x00B1, 0x00B2, 0x00B3,
    0x00B4, 0x00B5, 0x00B6, 0x00B7, 0x00B8, 0x00B9, 0x00BA, 0x00BB, 0x00BC, 0x00BD, 0x00BE, 0x00BF,
    0x00C0, 0x00C1, 0x00C2, 0x00C3, 0x00C4, 0x00C5, 0x00C6, 0x00C7, 0x00C8, 0x00C9, 0x00CA, 0x00CB,
    0x00CC, 0x00CD, 0x00CE, 0x00CF, 0x00D0, 0x00D1, 0x00D2, 0x00D3, 0x00D4, 0x00D5, 0x00D6, 0x00D7,
    0x00D8, 0x00D9, 0x00DA, 0x00DB, 0x00DC, 0x00DD, 0x00DE, 0x00DF, 0x00E0, 0x00E1, 0x00E2, 0x00E3,
    0x00E4, 0x00E5, 0x00E6, 0x00E7, 0x00E8, 0x00E9, 0x00EA, 0x00EB, 0x00EC, 0x00ED, 0x00EE, 0x00EF,
    0x00F0, 0x00F1, 0x00F2, 0x00F3, 0x00F4, 0x00F5, 0x00F6, 0x00F7, 0x00F8, 0x00F9, 0x00FA, 0x00FB,
    0x00FC, 0x00FD, 0x00FE, 0x00FF,
];

/// Code → Unicode (0 = no glyph).
pub(crate) const MAC_ROMAN: [u16; 256] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0020, 0x0021, 0x0022, 0x0023,
    0x0024, 0x0025, 0x0026, 0x0027, 0x0028, 0x0029, 0x002A, 0x002B, 0x002C, 0x002D, 0x002E, 0x002F,
    0x0030, 0x0031, 0x0032, 0x0033, 0x0034, 0x0035, 0x0036, 0x0037, 0x0038, 0x0039, 0x003A, 0x003B,
    0x003C, 0x003D, 0x003E, 0x003F, 0x0040, 0x0041, 0x0042, 0x0043, 0x0044, 0x0045, 0x0046, 0x0047,
    0x0048, 0x0049, 0x004A, 0x004B, 0x004C, 0x004D, 0x004E, 0x004F, 0x0050, 0x0051, 0x0052, 0x0053,
    0x0054, 0x0055, 0x0056, 0x0057, 0x0058, 0x0059, 0x005A, 0x005B, 0x005C, 0x005D, 0x005E, 0x005F,
    0x0060, 0x0061, 0x0062, 0x0063, 0x0064, 0x0065, 0x0066, 0x0067, 0x0068, 0x0069, 0x006A, 0x006B,
    0x006C, 0x006D, 0x006E, 0x006F, 0x0070, 0x0071, 0x0072, 0x0073, 0x0074, 0x0075, 0x0076, 0x0077,
    0x0078, 0x0079, 0x007A, 0x007B, 0x007C, 0x007D, 0x007E, 0x0000, 0x00C4, 0x00C5, 0x00C7, 0x00C9,
    0x00D1, 0x00D6, 0x00DC, 0x00E1, 0x00E0, 0x00E2, 0x00E4, 0x00E3, 0x00E5, 0x00E7, 0x00E9, 0x00E8,
    0x00EA, 0x00EB, 0x00ED, 0x00EC, 0x00EE, 0x00EF, 0x00F1, 0x00F3, 0x00F2, 0x00F4, 0x00F6, 0x00F5,
    0x00FA, 0x00F9, 0x00FB, 0x00FC, 0x2020, 0x00B0, 0x00A2, 0x00A3, 0x00A7, 0x2022, 0x00B6, 0x00DF,
    0x00AE, 0x00A9, 0x2122, 0x00B4, 0x00A8, 0x2260, 0x00C6, 0x00D8, 0x221E, 0x00B1, 0x2264, 0x2265,
    0x00A5, 0x00B5, 0x2202, 0x2211, 0x220F, 0x03C0, 0x222B, 0x00AA, 0x00BA, 0x2126, 0x00E6, 0x00F8,
    0x00BF, 0x00A1, 0x00AC, 0x221A, 0x0192, 0x2248, 0x2206, 0x00AB, 0x00BB, 0x2026, 0x0020, 0x00C0,
    0x00C3, 0x00D5, 0x0152, 0x0153, 0x2013, 0x2014, 0x201C, 0x201D, 0x2018, 0x2019, 0x00F7, 0x25CA,
    0x00FF, 0x0178, 0x2044, 0x00A4, 0x2039, 0x203A, 0xFB01, 0xFB02, 0x2021, 0x00B7, 0x201A, 0x201E,
    0x2030, 0x00C2, 0x00CA, 0x00C1, 0x00CB, 0x00C8, 0x00CD, 0x00CE, 0x00CF, 0x00CC, 0x00D3, 0x00D4,
    0xF8FF, 0x00D2, 0x00DA, 0x00DB, 0x00D9, 0x0131, 0x02C6, 0x02DC, 0x00AF, 0x02D8, 0x02D9, 0x02DA,
    0x00B8, 0x02DD, 0x02DB, 0x02C7,
];

/// Glyph names, sorted, separated by newlines.
const NAMES: &str = "\
A\nAE\nAEacute\nAEmacron\nAEsmall\nAacute\nAacutesmall\nAbreve\nAbreveacute\n\
Abrevecyrillic\nAbrevedotbelow\nAbrevegrave\nAbrevehookabove\nAbrevetilde\nAcaron\n\
Acircle\nAcircumflex\nAcircumflexacute\nAcircumflexdotbelow\nAcircumflexgrave\n\
Acircumflexhookabove\nAcircumflexsmall\nAcircumflextilde\nAcute\nAcutesmall\nAcyrillic\n\
Adblgrave\nAdieresis\nAdieresiscyrillic\nAdieresismacron\nAdieresissmall\nAdotbelow\n\
Adotmacron\nAgrave\nAgravesmall\nAhookabove\nAiecyrillic\nAinvertedbreve\nAlpha\n\
Alphatonos\nAmacron\nAmonospace\nAogonek\nAring\nAringacute\nAringbelow\nAringsmall\n\
Asmall\nAtilde\nAtildesmall\nAybarmenian\nB\nBcircle\nBdotaccent\nBdotbelow\nBecyrillic\n\
Benarmenian\nBeta\nBhook\nBlinebelow\nBmonospace\nBrevesmall\nBsmall\nBtopbar\nC\n\
Caarmenian\nCacute\nCaron\nCaronsmall\nCcaron\nCcedilla\nCcedillaacute\nCcedillasmall\n\
Ccircle\nCcircumflex\nCdot\nCdotaccent\nCedillasmall\nChaarmenian\nCheabkhasiancyrillic\n\
Checyrillic\nChedescenderabkhasiancyrillic\nChedescendercyrillic\nChedieresiscyrillic\n\
Cheharmenian\nChekhakassiancyrillic\nCheverticalstrokecyrillic\nChi\nChook\n\
Circumflexsmall\nCmonospace\nCoarmenian\nCsmall\nD\nDZ\nDZcaron\nDaarmenian\nDafrican\n\
Dbar\nDcaron\nDcedilla\nDcircle\nDcircumflexbelow\nDcroat\nDdotaccent\nDdotbelow\n\
Decyrillic\nDeicoptic\nDelta\nDeltagreek\nDhook\nDieresis\nDieresisAcute\nDieresisGrave\n\
Dieresissmall\nDigammagreek\nDjecyrillic\nDlinebelow\nDmonospace\nDotaccentsmall\nDslash\n\
Dsmall\nDtopbar\nDz\nDzcaron\nDzeabkhasiancyrillic\nDzecyrillic\nDzhecyrillic\nE\nEacute\n\
Eacutesmall\nEbreve\nEcaron\nEcedillabreve\nEcharmenian\nEcircle\nEcircumflex\n\
Ecircumflexacute\nEcircumflexbelow\nEcircumflexdotbelow\nEcircumflexgrave\n\
Ecircumflexhookabove\nEcircumflexsmall\nEcircumflextilde\nEcyrillic\nEdblgrave\n\
Edieresis\nEdieresissmall\nEdot\nEdotaccent\nEdotbelow\nEfcyrillic\nEgrave\nEgravesmall\n\
Eharmenian\nEhookabove\nEightroman\nEinvertedbreve\nEiotifiedcyrillic\nElcyrillic\n\
Elevenroman\nEmacron\nEmacronacute\nEmacrongrave\nEmcyrillic\nEmonospace\nEncyrillic\n\
Endescendercyrillic\nEng\nEnghecyrillic\nEnhookcyrillic\nEogonek\nEopen\nEpsilon\n\
Epsilontonos\nErcyrillic\nEreversed\nEreversedcyrillic\nEscyrillic\nEsdescendercyrillic\n\
Esh\nEsmall\nEta\nEtarmenian\nEtatonos\nEth\nEthsmall\nEtilde\nEtildebelow\nEuro\nEzh\n\
Ezhcaron\nEzhreversed\nF\nFFIsmall\nFFLsmall\nFFsmall\nFIsmall\nFLsmall\nFcircle\n\
Fdotaccent\nFeharmenian\nFeicoptic\nFhook\nFitacyrillic\nFiveroman\nFmonospace\n\
Fourroman\nFsmall\nG\nGBsquare\nGacute\nGamma\nGammaafrican\nGangiacoptic\nGbreve\n\
Gcaron\nGcedilla\nGcircle\nGcircumflex\nGcommaaccent\nGdot\nGdotaccent\nGecyrillic\n\
Germandbls\nGermandbls2\nGermandblssmall\nGhadarmenian\nGhemiddlehookcyrillic\n\
Ghestrokecyrillic\nGheupturncyrillic\nGhook\nGimarmenian\nGjecyrillic\nGmacron\n\
Gmonospace\nGrave\nGravesmall\nGsmall\nGsmallhook\nGstroke\nH\nH18533\nH18543\nH18551\n\
H22073\nHPsquare\nHaabkhasiancyrillic\nHadescendercyrillic\nHardsigncyrillic\nHbar\n\
Hbrevebelow\nHcedilla\nHcircle\nHcircumflex\nHdieresis\nHdotaccent\nHdotbelow\n\
Hmonospace\nHoarmenian\nHoricoptic\nHsmall\nHungarumlaut\nHungarumlautsmall\nHzsquare\nI\n\
IAcyrillic\nIJ\nIUcyrillic\nIacute\nIacutesmall\nIbreve\nIcaron\nIcircle\nIcircumflex\n\
Icircumflexsmall\nIcyrillic\nIdblgrave\nIdieresis\nIdieresisacute\nIdieresiscyrillic\n\
Idieresissmall\nIdot\nIdotaccent\nIdotbelow\nIebrevecyrillic\nIecyrillic\nIfractur\n\
Ifraktur\nIgrave\nIgravesmall\nIhookabove\nIicyrillic\nIinvertedbreve\nIishortcyrillic\n\
Imacron\nImacroncyrillic\nImonospace\nIniarmenian\nIocyrillic\nIogonek\nIota\n\
Iotaafrican\nIotadieresis\nIotatonos\nIsmall\nIstroke\nItilde\nItildebelow\n\
Izhitsacyrillic\nIzhitsadblgravecyrillic\nJ\nJaarmenian\nJcircle\nJcircumflex\n\
Jecyrillic\nJheharmenian\nJmonospace\nJsmall\nK\nKBsquare\nKKsquare\nKabashkircyrillic\n\
Kacute\nKacyrillic\nKadescendercyrillic\nKahookcyrillic\nKappa\nKastrokecyrillic\n\
Kaverticalstrokecyrillic\nKcaron\nKcedilla\nKcircle\nKcommaaccent\nKdotbelow\n\
Keharmenian\nKenarmenian\nKhacyrillic\nKheicoptic\nKhook\nKjecyrillic\nKlinebelow\n\
Kmonospace\nKoppacyrillic\nKoppagreek\nKsicyrillic\nKsmall\nL\nLJ\nLL\nLacute\nLambda\n\
Lcaron\nLcedilla\nLcircle\nLcircumflexbelow\nLcommaaccent\nLdot\nLdotaccent\nLdotbelow\n\
Ldotbelowmacron\nLiwnarmenian\nLj\nLjecyrillic\nLlinebelow\nLmonospace\nLslash\n\
Lslashsmall\nLsmall\nM\nMBsquare\nMacron\nMacronsmall\nMacute\nMcircle\nMdotaccent\n\
Mdotbelow\nMenarmenian\nMmonospace\nMsmall\nMturned\nMu\nN\nNJ\nNacute\nNcaron\nNcedilla\n\
Ncircle\nNcircumflexbelow\nNcommaaccent\nNdotaccent\nNdotbelow\nNg\nNhookleft\nNineroman\n\
Nj\nNjecyrillic\nNlinebelow\nNmonospace\nNowarmenian\nNsmall\nNtilde\nNtildesmall\nNu\nO\n\
OE\nOEsmall\nOacute\nOacutesmall\nObarredcyrillic\nObarreddieresiscyrillic\nObreve\n\
Ocaron\nOcenteredtilde\nOcircle\nOcircumflex\nOcircumflexacute\nOcircumflexdotbelow\n\
Ocircumflexgrave\nOcircumflexhookabove\nOcircumflexsmall\nOcircumflextilde\nOcyrillic\n\
Odblacute\nOdblgrave\nOdieresis\nOdieresiscyrillic\nOdieresissmall\nOdotbelow\n\
Ogoneksmall\nOgrave\nOgravesmall\nOharmenian\nOhm\nOhookabove\nOhorn\nOhornacute\n\
Ohorndotbelow\nOhorngrave\nOhornhookabove\nOhorntilde\nOhungarumlaut\nOi\nOinvertedbreve\n\
Omacron\nOmacronacute\nOmacrongrave\nOmega\nOmegacyrillic\nOmegagreek\n\
Omegaroundcyrillic\nOmegatitlocyrillic\nOmegatonos\nOmicron\nOmicrontonos\nOmonospace\n\
Oneroman\nOogonek\nOogonekmacron\nOopen\nOslash\nOslashacute\nOslashsmall\nOsmall\n\
Ostrokeacute\nOtcyrillic\nOtilde\nOtildeacute\nOtildedieresis\nOtildesmall\nP\nPacute\n\
Pcircle\nPdotaccent\nPecyrillic\nPeharmenian\nPemiddlehookcyrillic\nPhi\nPhook\nPi\n\
Piwrarmenian\nPmonospace\nPsi\nPsicyrillic\nPsmall\nQ\nQcircle\nQmonospace\nQsmall\nR\n\
Raarmenian\nRacute\nRcaron\nRcedilla\nRcircle\nRcommaaccent\nRdblgrave\nRdotaccent\n\
Rdotbelow\nRdotbelowmacron\nReharmenian\nRfractur\nRfraktur\nRho\nRingsmall\n\
Rinvertedbreve\nRlinebelow\nRmonospace\nRsmall\nRsmallinverted\nRsmallinvertedsuperior\n\
S\nSF010000\nSF020000\nSF030000\nSF040000\nSF050000\nSF060000\nSF070000\nSF080000\n\
SF090000\nSF100000\nSF110000\nSF190000\nSF200000\nSF210000\nSF220000\nSF230000\nSF240000\n\
SF250000\nSF260000\nSF270000\nSF280000\nSF360000\nSF370000\nSF380000\nSF390000\nSF400000\n\
SF410000\nSF420000\nSF430000\nSF440000\nSF450000\nSF460000\nSF470000\nSF480000\nSF490000\n\
SF500000\nSF510000\nSF520000\nSF530000\nSF540000\nSS\nSSsmall\nSacute\nSacutedotaccent\n\
Sampigreek\nScaron\nScarondotaccent\nScaronsmall\nScedilla\nSchwa\nSchwacyrillic\n\
Schwadieresiscyrillic\nScircle\nScircumflex\nScommaaccent\nSdotaccent\nSdotbelow\n\
Sdotbelowdotaccent\nSeharmenian\nSevenroman\nShaarmenian\nShacyrillic\nShchacyrillic\n\
Sheicoptic\nShhacyrillic\nShimacoptic\nSigma\nSixroman\nSmonospace\nSoftsigncyrillic\n\
Ssmall\nStigmagreek\nT\nTau\nTbar\nTcaron\nTcedilla\nTcircle\nTcircumflexbelow\n\
Tcommaaccent\nTdotaccent\nTdotbelow\nTecyrillic\nTedescendercyrillic\nTenroman\n\
Tetsecyrillic\nTheta\nThook\nThorn\nThornsmall\nThreeroman\nTildesmall\nTiwnarmenian\n\
Tlinebelow\nTmonospace\nToarmenian\nTonefive\nTonesix\nTonetwo\nTretroflexhook\n\
Tsecyrillic\nTshecyrillic\nTsmall\nTwelveroman\nTworoman\nU\nUacute\nUacutesmall\nUbreve\n\
Ucaron\nUcircle\nUcircumflex\nUcircumflexbelow\nUcircumflexsmall\nUcyrillic\nUdblacute\n\
Udblgrave\nUdieresis\nUdieresisacute\nUdieresisbelow\nUdieresiscaron\nUdieresiscyrillic\n\
Udieresisgrave\nUdieresismacron\nUdieresissmall\nUdotbelow\nUgrave\nUgravesmall\n\
Uhookabove\nUhorn\nUhornacute\nUhorndotbelow\nUhorngrave\nUhornhookabove\nUhorntilde\n\
Uhungarumlaut\nUhungarumlautcyrillic\nUinvertedbreve\nUkcyrillic\nUmacron\n\
Umacroncyrillic\nUmacrondieresis\nUmonospace\nUogonek\nUpsilon\nUpsilon1\n\
Upsilonacutehooksymbolgreek\nUpsilonafrican\nUpsilondieresis\n\
Upsilondieresishooksymbolgreek\nUpsilonhooksymbol\nUpsilontonos\nUring\nUshortcyrillic\n\
Usmall\nUstraightcyrillic\nUstraightstrokecyrillic\nUtilde\nUtildeacute\nUtildebelow\nV\n\
Vcircle\nVdotbelow\nVecyrillic\nVewarmenian\nVhook\nVmonospace\nVoarmenian\nVsmall\n\
Vtilde\nW\nWacute\nWcircle\nWcircumflex\nWdieresis\nWdotaccent\nWdotbelow\nWgrave\n\
Wmonospace\nWsmall\nX\nXcircle\nXdieresis\nXdotaccent\nXeharmenian\nXi\nXmonospace\n\
Xsmall\nY\nYacute\nYacutesmall\nYatcyrillic\nYcircle\nYcircumflex\nYdieresis\n\
Ydieresissmall\nYdotaccent\nYdotbelow\nYericyrillic\nYerudieresiscyrillic\nYgrave\nYhook\n\
Yhookabove\nYiarmenian\nYicyrillic\nYiwnarmenian\nYmonospace\nYsmall\nYtilde\n\
Yusbigcyrillic\nYusbigiotifiedcyrillic\nYuslittlecyrillic\nYuslittleiotifiedcyrillic\nZ\n\
Zaarmenian\nZacute\nZcaron\nZcaronsmall\nZcircle\nZcircumflex\nZdot\nZdotaccent\n\
Zdotbelow\nZecyrillic\nZedescendercyrillic\nZedieresiscyrillic\nZeta\nZhearmenian\n\
Zhebrevecyrillic\nZhecyrillic\nZhedescendercyrillic\nZhedieresiscyrillic\nZlinebelow\n\
Zmonospace\nZsmall\nZstroke\na\naabengali\naacute\naadeva\naagujarati\naagurmukhi\n\
aamatragurmukhi\naarusquare\naavowelsignbengali\naavowelsigndeva\naavowelsigngujarati\n\
abbreviationmarkarmenian\nabbreviationsigndeva\nabengali\nabopomofo\nabreve\nabreveacute\n\
abrevecyrillic\nabrevedotbelow\nabrevegrave\nabrevehookabove\nabrevetilde\nacaron\n\
acircle\nacircumflex\nacircumflexacute\nacircumflexdotbelow\nacircumflexgrave\n\
acircumflexhookabove\nacircumflextilde\nacute\nacutebelowcmb\nacutecmb\nacutecomb\n\
acutedeva\nacutelowmod\nacutetonecmb\nacyrillic\nadblgrave\naddakgurmukhi\nadeva\n\
adieresis\nadieresiscyrillic\nadieresismacron\nadotbelow\nadotmacron\nae\naeacute\n\
aekorean\naemacron\nafii00208\nafii08941\nafii10017\nafii10018\nafii10019\nafii10020\n\
afii10021\nafii10022\nafii10023\nafii10024\nafii10025\nafii10026\nafii10027\nafii10028\n\
afii10029\nafii10030\nafii10031\nafii10032\nafii10033\nafii10034\nafii10035\nafii10036\n\
afii10037\nafii10038\nafii10039\nafii10040\nafii10041\nafii10042\nafii10043\nafii10044\n\
afii10045\nafii10046\nafii10047\nafii10048\nafii10049\nafii10050\nafii10051\nafii10052\n\
afii10053\nafii10054\nafii10055\nafii10056\nafii10057\nafii10058\nafii10059\nafii10060\n\
afii10061\nafii10062\nafii10063\nafii10064\nafii10065\nafii10066\nafii10067\nafii10068\n\
afii10069\nafii10070\nafii10071\nafii10072\nafii10073\nafii10074\nafii10075\nafii10076\n\
afii10077\nafii10078\nafii10079\nafii10080\nafii10081\nafii10082\nafii10083\nafii10084\n\
afii10085\nafii10086\nafii10087\nafii10088\nafii10089\nafii10090\nafii10091\nafii10092\n\
afii10093\nafii10094\nafii10095\nafii10096\nafii10097\nafii10098\nafii10099\nafii10100\n\
afii10101\nafii10102\nafii10103\nafii10104\nafii10105\nafii10106\nafii10107\nafii10108\n\
afii10109\nafii10110\nafii10145\nafii10146\nafii10147\nafii10148\nafii10192\nafii10193\n\
afii10194\nafii10195\nafii10196\nafii10831\nafii10832\nafii10846\nafii299\nafii300\n\
afii301\nafii57381\nafii57388\nafii57392\nafii57393\nafii57394\nafii57395\nafii57396\n\
afii57397\nafii57398\nafii57399\nafii57400\nafii57401\nafii57403\nafii57407\nafii57409\n\
afii57410\nafii57411\nafii57412\nafii57413\nafii57414\nafii57415\nafii57416\nafii57417\n\
afii57418\nafii57419\nafii57420\nafii57421\nafii57422\nafii57423\nafii57424\nafii57425\n\
afii57426\nafii57427\nafii57428\nafii57429\nafii57430\nafii57431\nafii57432\nafii57433\n\
afii57434\nafii57440\nafii57441\nafii57442\nafii57443\nafii57444\nafii57445\nafii57446\n\
afii57448\nafii57449\nafii57450\nafii57451\nafii57452\nafii57453\nafii57454\nafii57455\n\
afii57456\nafii57457\nafii57458\nafii57470\nafii57505\nafii57506\nafii57507\nafii57508\n\
afii57509\nafii57511\nafii57512\nafii57513\nafii57514\nafii57519\nafii57534\nafii57636\n\
afii57645\nafii57658\nafii57664\nafii57665\nafii57666\nafii57667\nafii57668\nafii57669\n\
afii57670\nafii57671\nafii57672\nafii57673\nafii57674\nafii57675\nafii57676\nafii57677\n\
afii57678\nafii57679\nafii57680\nafii57681\nafii57682\nafii57683\nafii57684\nafii57685\n\
afii57686\nafii57687\nafii57688\nafii57689\nafii57690\nafii57694\nafii57695\nafii57700\n\
afii57705\nafii57716\nafii57717\nafii57718\nafii57723\nafii57793\nafii57794\nafii57795\n\
afii57796\nafii57797\nafii57798\nafii57799\nafii57800\nafii57801\nafii57802\nafii57803\n\
afii57804\nafii57806\nafii57807\nafii57839\nafii57841\nafii57842\nafii57929\nafii61248\n\
afii61289\nafii61352\nafii61573\nafii61574\nafii61575\nafii61664\nafii63167\nafii64937\n\
agrave\nagujarati\nagurmukhi\nahiragana\nahookabove\naibengali\naibopomofo\naideva\n\
aiecyrillic\naigujarati\naigurmukhi\naimatragurmukhi\nainarabic\nainfinalarabic\n\
aininitialarabic\nainmedialarabic\nainvertedbreve\naivowelsignbengali\naivowelsigndeva\n\
aivowelsigngujarati\nakatakana\nakatakanahalfwidth\nakorean\nalef\nalefarabic\n\
alefdageshhebrew\naleffinalarabic\nalefhamzaabovearabic\nalefhamzaabovefinalarabic\n\
alefhamzabelowarabic\nalefhamzabelowfinalarabic\nalefhebrew\naleflamedhebrew\n\
alefmaddaabovearabic\nalefmaddaabovefinalarabic\nalefmaksuraarabic\n\
alefmaksurafinalarabic\nalefmaksurainitialarabic\nalefmaksuramedialarabic\n\
alefpatahhebrew\nalefqamatshebrew\naleph\nallequal\nalpha\nalphatonos\naltselector\n\
amacron\namonospace\nampersand\nampersandmonospace\nampersandsmall\namsquare\nanbopomofo\n\
angbopomofo\nangbracketleft\nangbracketright\nangkhankhuthai\nangle\nanglebracketleft\n\
anglebracketleftvertical\nanglebracketright\nanglebracketrightvertical\nangleleft\n\
angleleftBig\nangleleftBigg\nangleleftbig\nangleleftbigg\nangleright\nanglerightBig\n\
anglerightBigg\nanglerightbig\nanglerightbigg\nangstrom\nanoteleia\nanudattadeva\n\
anusvarabengali\nanusvaradeva\nanusvaragujarati\naogonek\napaatosquare\naparen\n\
apostrophearmenian\napostrophemod\napple\napproaches\napproxequal\napproxequalorimage\n\
approximatelyequal\naraeaekorean\naraeakorean\narc\narighthalfring\naring\naringacute\n\
aringbelow\narrowboth\narrowbothv\narrowbt\narrowdashdown\narrowdashleft\narrowdashright\n\
arrowdashup\narrowdblboth\narrowdblbothv\narrowdbldown\narrowdblleft\narrowdblright\n\
arrowdbltp\narrowdblup\narrowdblvertex\narrowdown\narrowdownleft\narrowdownright\n\
arrowdownwhite\narrowheaddownmod\narrowheadleftmod\narrowheadrightmod\narrowheadupmod\n\
arrowhorizex\narrowleft\narrowleftbothalf\narrowleftdbl\narrowleftdblstroke\n\
arrowleftoverright\narrowlefttophalf\narrowleftwhite\narrownortheast\narrownorthwest\n\
arrowright\narrowrightbothalf\narrowrightdblstroke\narrowrightheavy\narrowrightoverleft\n\
arrowrighttophalf\narrowrightwhite\narrowsoutheast\narrowsouthwest\narrowtableft\n\
arrowtabright\narrowtp\narrowup\narrowupdn\narrowupdnbse\narrowupdownbase\narrowupleft\n\
arrowupleftofdown\narrowupright\narrowupwhite\narrowvertex\narrowvertex2\n\
ascendercompwordmark\nasciicircum\nasciicircummonospace\nasciitilde\nasciitildemonospace\n\
ascript\nascriptturned\nasmallhiragana\nasmallkatakana\nasmallkatakanahalfwidth\n\
asterisk\nasteriskaltonearabic\nasteriskarabic\nasteriskcentered\nasteriskmath\n\
asteriskmonospace\nasterisksmall\nasterism\nasuperior\nasymptoticallyequal\nat\natilde\n\
atmonospace\natsmall\naturned\naubengali\naubopomofo\naudeva\naugujarati\naugurmukhi\n\
aulengthmarkbengali\naumatragurmukhi\nauvowelsignbengali\nauvowelsigndeva\n\
auvowelsigngujarati\navagrahadeva\naybarmenian\nayin\nayinaltonehebrew\nayinhebrew\nb\n\
babengali\nbackslash\nbackslashBig\nbackslashBigg\nbackslashbig\nbackslashbigg\n\
backslashmonospace\nbadeva\nbagujarati\nbagurmukhi\nbahiragana\nbahtthai\nbakatakana\n\
bar\nbardbl\nbardblex\nbarex\nbarmonospace\nbbopomofo\nbcircle\nbdotaccent\nbdotbelow\n\
beamedsixteenthnotes\nbecause\nbecyrillic\nbeharabic\nbehfinalarabic\nbehinitialarabic\n\
behiragana\nbehmedialarabic\nbehmeeminitialarabic\nbehmeemisolatedarabic\n\
behnoonfinalarabic\nbekatakana\nbenarmenian\nbet\nbeta\nbetasymbolgreek\nbetdagesh\n\
betdageshhebrew\nbethebrew\nbetrafehebrew\nbhabengali\nbhadeva\nbhagujarati\nbhagurmukhi\n\
bhook\nbihiragana\nbikatakana\nbilabialclick\nbindigurmukhi\nbirusquare\nblackcircle\n\
blackdiamond\nblackdownpointingtriangle\nblackleftpointingpointer\n\
blackleftpointingtriangle\nblacklenticularbracketleft\n\
blacklenticularbracketleftvertical\nblacklenticularbracketright\n\
blacklenticularbracketrightvertical\nblacklowerlefttriangle\nblacklowerrighttriangle\n\
blackrectangle\nblackrightpointingpointer\nblackrightpointingtriangle\nblacksmallsquare\n\
blacksmilingface\nblacksquare\nblackstar\nblackupperlefttriangle\n\
blackupperrighttriangle\nblackuppointingsmalltriangle\nblackuppointingtriangle\nblank\n\
blinebelow\nblock\nbmonospace\nbobaimaithai\nbohiragana\nbokatakana\nbparen\nbqsquare\n\
braceex\nbraceex2\nbraceleft\nbraceleftBig\nbraceleftBigg\nbraceleftbig\nbraceleftbigg\n\
braceleftbt\nbraceleftmid\nbraceleftmid2\nbraceleftmonospace\nbraceleftsmall\n\
bracelefttp\nbraceleftvertical\nbraceright\nbracerightBig\nbracerightBigg\nbracerightbig\n\
bracerightbigg\nbracerightbt\nbracerightmid\nbracerightmid2\nbracerightmonospace\n\
bracerightsmall\nbracerighttp\nbracerightvertical\nbracketleft\nbracketleftBig\n\
bracketleftBigg\nbracketleftbig\nbracketleftbigg\nbracketleftbt\nbracketleftex\n\
bracketleftmonospace\nbracketlefttp\nbracketright\nbracketrightBig\nbracketrightBigg\n\
bracketrightbig\nbracketrightbigg\nbracketrightbt\nbracketrightex\nbracketrightmonospace\n\
bracketrighttp\nbreve\nbrevebelowcmb\nbrevecmb\nbreveinvertedbelowcmb\nbreveinvertedcmb\n\
breveinverteddoublecmb\nbridgebelowcmb\nbridgeinvertedbelowcmb\nbrokenbar\nbstroke\n\
bsuperior\nbtopbar\nbuhiragana\nbukatakana\nbullet\nbulletinverse\nbulletoperator\n\
bullseye\nc\ncaarmenian\ncabengali\ncacute\ncadeva\ncagujarati\ncagurmukhi\ncalsquare\n\
candrabindubengali\ncandrabinducmb\ncandrabindudeva\ncandrabindugujarati\n\
capitalcompwordmark\ncapslock\ncareof\ncaron\ncaronbelowcmb\ncaroncmb\ncarriagereturn\n\
cbopomofo\nccaron\nccedilla\nccedillaacute\nccircle\nccircumflex\nccurl\ncdot\n\
cdotaccent\ncdsquare\ncedilla\ncedillacmb\nceilingleft\nceilingleftBig\nceilingleftBigg\n\
ceilingleftbig\nceilingleftbigg\nceilingright\nceilingrightBig\nceilingrightBigg\n\
ceilingrightbig\nceilingrightbigg\ncent\ncentigrade\ncentinferior\ncentmonospace\n\
centoldstyle\ncentsuperior\nchaarmenian\nchabengali\nchadeva\nchagujarati\nchagurmukhi\n\
chbopomofo\ncheabkhasiancyrillic\ncheckmark\nchecyrillic\nchedescenderabkhasiancyrillic\n\
chedescendercyrillic\nchedieresiscyrillic\ncheharmenian\nchekhakassiancyrillic\n\
cheverticalstrokecyrillic\nchi\nchieuchacirclekorean\nchieuchaparenkorean\n\
chieuchcirclekorean\nchieuchkorean\nchieuchparenkorean\nchochangthai\nchochanthai\n\
chochingthai\nchochoethai\nchook\ncieucacirclekorean\ncieucaparenkorean\n\
cieuccirclekorean\ncieuckorean\ncieucparenkorean\ncieucuparenkorean\ncircle\n\
circlecopyrt\ncircledivide\ncircledot\ncircledotdisplay\ncircledottext\ncircleminus\n\
circlemultiply\ncirclemultiplydisplay\ncirclemultiplytext\ncircleot\ncircleplus\n\
circleplusdisplay\ncircleplustext\ncirclepostalmark\ncirclewithlefthalfblack\n\
circlewithrighthalfblack\ncircumflex\ncircumflexbelowcmb\ncircumflexcmb\nclear\n\
clickalveolar\nclickdental\nclicklateral\nclickretroflex\nclub\nclubsuitblack\n\
clubsuitwhite\ncmcubedsquare\ncmonospace\ncmsquaredsquare\ncoarmenian\ncolon\n\
colonmonetary\ncolonmonospace\ncolonsign\ncolonsmall\ncolontriangularhalfmod\n\
colontriangularmod\ncomma\ncommaabovecmb\ncommaaboverightcmb\ncommaaccent\ncommaarabic\n\
commaarmenian\ncommainferior\ncommamonospace\ncommareversedabovecmb\ncommareversedmod\n\
commasmall\ncommasuperior\ncommaturnedabovecmb\ncommaturnedmod\ncompass\ncompwordmark\n\
congruent\ncontintegraldisplay\ncontintegraltext\ncontourintegral\ncontrol\ncontrolACK\n\
controlBEL\ncontrolBS\ncontrolCAN\ncontrolCR\ncontrolDC1\ncontrolDC2\ncontrolDC3\n\
controlDC4\ncontrolDEL\ncontrolDLE\ncontrolEM\ncontrolENQ\ncontrolEOT\ncontrolESC\n\
controlETB\ncontrolETX\ncontrolFF\ncontrolFS\ncontrolGS\ncontrolHT\ncontrolLF\n\
controlNAK\ncontrolRS\ncontrolSI\ncontrolSO\ncontrolSOT\ncontrolSTX\ncontrolSUB\n\
controlSYN\ncontrolUS\ncontrolVT\ncoproduct\ncoproductdisplay\ncoproducttext\ncopyright\n\
copyrightsans\ncopyrightserif\ncornerbracketleft\ncornerbracketlefthalfwidth\n\
cornerbracketleftvertical\ncornerbracketright\ncornerbracketrighthalfwidth\n\
cornerbracketrightvertical\ncorporationsquare\ncosquare\ncoverkgsquare\ncparen\ncruzeiro\n\
cstretched\ncurlyand\ncurlyor\ncurrency\ncwm\ncyrBreve\ncyrFlex\ncyrbreve\ncyrflex\nd\n\
daarmenian\ndabengali\ndadarabic\ndadeva\ndadfinalarabic\ndadinitialarabic\n\
dadmedialarabic\ndagesh\ndageshhebrew\ndagger\ndaggerdbl\ndagujarati\ndagurmukhi\n\
dahiragana\ndakatakana\ndalarabic\ndalet\ndaletdagesh\ndaletdageshhebrew\n\
dalethatafpatah\ndalethatafpatahhebrew\ndalethatafsegol\ndalethatafsegolhebrew\n\
dalethebrew\ndalethiriq\ndalethiriqhebrew\ndaletholam\ndaletholamhebrew\ndaletpatah\n\
daletpatahhebrew\ndaletqamats\ndaletqamatshebrew\ndaletqubuts\ndaletqubutshebrew\n\
daletsegol\ndaletsegolhebrew\ndaletsheva\ndaletshevahebrew\ndalettsere\ndalettserehebrew\n\
dalfinalarabic\ndammaarabic\ndammalowarabic\ndammatanaltonearabic\ndammatanarabic\ndanda\n\
dargahebrew\ndargalefthebrew\ndasiapneumatacyrilliccmb\ndbar\ndblGrave\n\
dblanglebracketleft\ndblanglebracketleftvertical\ndblanglebracketright\n\
dblanglebracketrightvertical\ndblarchinvertedbelowcmb\ndblarrowleft\ndblarrowright\n\
dblbracketleft\ndblbracketright\ndbldanda\ndblgrave\ndblgravecmb\ndblintegral\n\
dbllowline\ndbllowlinecmb\ndbloverlinecmb\ndblprimemod\ndblverticalbar\n\
dblverticallineabovecmb\ndbopomofo\ndbsquare\ndcaron\ndcedilla\ndcircle\n\
dcircumflexbelow\ndcroat\nddabengali\nddadeva\nddagujarati\nddagurmukhi\nddalarabic\n\
ddalfinalarabic\ndddhadeva\nddhabengali\nddhadeva\nddhagujarati\nddhagurmukhi\n\
ddotaccent\nddotbelow\ndecimalseparatorarabic\ndecimalseparatorpersian\ndecyrillic\n\
degree\ndehihebrew\ndehiragana\ndeicoptic\ndekatakana\ndeleteleft\ndeleteright\ndelta\n\
deltaturned\ndenominatorminusonenumeratorbengali\ndezh\ndhabengali\ndhadeva\ndhagujarati\n\
dhagurmukhi\ndhook\ndialytikatonos\ndialytikatonoscmb\ndiamond\ndiamond2\ndiamondmath\n\
diamondsuitwhite\ndieresis\ndieresisacute\ndieresisbelowcmb\ndieresiscmb\ndieresisgrave\n\
dieresistonos\ndihiragana\ndikatakana\ndittomark\ndivide\ndivides\ndivisionslash\n\
djecyrillic\ndkshade\ndlinebelow\ndlsquare\ndmacron\ndmonospace\ndnblock\ndochadathai\n\
dodekthai\ndohiragana\ndokatakana\ndollar\ndollarinferior\ndollarmonospace\n\
dollaroldstyle\ndollarsmall\ndollarsuperior\ndong\ndorusquare\ndotaccent\ndotaccentcmb\n\
dotbelowcmb\ndotbelowcomb\ndotkatakana\ndotlessi\ndotlessj\ndotlessj2\n\
dotlessjstrokehook\ndotmath\ndottedcircle\ndoubleyodpatah\ndoubleyodpatahhebrew\n\
downtackbelowcmb\ndowntackmod\ndparen\ndsuperior\ndtail\ndtopbar\nduhiragana\ndukatakana\n\
dz\ndzaltone\ndzcaron\ndzcurl\ndzeabkhasiancyrillic\ndzecyrillic\ndzhecyrillic\ne\n\
eacute\nearth\nebengali\nebopomofo\nebreve\necandradeva\necandragujarati\n\
ecandravowelsigndeva\necandravowelsigngujarati\necaron\necedillabreve\necharmenian\n\
echyiwnarmenian\necircle\necircumflex\necircumflexacute\necircumflexbelow\n\
ecircumflexdotbelow\necircumflexgrave\necircumflexhookabove\necircumflextilde\necyrillic\n\
edblgrave\nedeva\nedieresis\nedot\nedotaccent\nedotbelow\neegurmukhi\neematragurmukhi\n\
efcyrillic\negrave\negujarati\neharmenian\nehbopomofo\nehiragana\nehookabove\neibopomofo\n\
eight\neightarabic\neightbengali\neightcircle\neightcircleinversesansserif\neightdeva\n\
eighteencircle\neighteenparen\neighteenperiod\neightgujarati\neightgurmukhi\n\
eighthackarabic\neighthangzhou\neighthnotebeamed\neightideographicparen\neightinferior\n\
eightmonospace\neightoldstyle\neightparen\neightperiod\neightpersian\neightroman\n\
eightsuperior\neightthai\neinvertedbreve\neiotifiedcyrillic\nekatakana\n\
ekatakanahalfwidth\nekonkargurmukhi\nekorean\nelcyrillic\nelement\nelevencircle\n\
elevenparen\nelevenperiod\nelevenroman\nellipsis\nellipsisvertical\nemacron\n\
emacronacute\nemacrongrave\nemcyrillic\nemdash\nemdashvertical\nemonospace\n\
emphasismarkarmenian\nemptyset\nemptyslot\nenbopomofo\nencyrillic\nendash\n\
endashvertical\nendescendercyrillic\neng\nengbopomofo\nenghecyrillic\nenhookcyrillic\n\
enspace\neogonek\neokorean\neopen\neopenclosed\neopenreversed\neopenreversedclosed\n\
eopenreversedhook\neparen\nepsilon\nepsilon1\nepsilontonos\nequal\nequalmonospace\n\
equalsmall\nequalsuperior\nequivalence\nequivasymptotic\nerbopomofo\nercyrillic\n\
ereversed\nereversedcyrillic\nescyrillic\nesdescendercyrillic\nesh\neshcurl\neshortdeva\n\
eshortvowelsigndeva\neshreversedloop\neshsquatreversed\nesmallhiragana\nesmallkatakana\n\
esmallkatakanahalfwidth\nestimated\nesuperior\neta\netarmenian\netatonos\neth\netilde\n\
etildebelow\netnahtafoukhhebrew\netnahtafoukhlefthebrew\netnahtahebrew\n\
etnahtalefthebrew\neturned\neukorean\neuro\nevowelsignbengali\nevowelsigndeva\n\
evowelsigngujarati\nexclam\nexclamarmenian\nexclamdbl\nexclamdown\nexclamdownsmall\n\
exclammonospace\nexclamsmall\nexistential\nezh\nezhcaron\nezhcurl\nezhreversed\nezhtail\n\
f\nf_f\nf_f_i\nf_f_l\nf_i\nf_l\nfadeva\nfagurmukhi\nfahrenheit\nfathaarabic\n\
fathalowarabic\nfathatanarabic\nfbopomofo\nfcircle\nfdotaccent\nfeharabic\nfeharmenian\n\
fehfinalarabic\nfehinitialarabic\nfehmedialarabic\nfeicoptic\nfemale\nff\nffi\nffl\nfi\n\
fifteencircle\nfifteenparen\nfifteenperiod\nfiguredash\nfilledbox\nfilledrect\nfinalkaf\n\
finalkafdagesh\nfinalkafdageshhebrew\nfinalkafhebrew\nfinalkafqamats\n\
finalkafqamatshebrew\nfinalkafsheva\nfinalkafshevahebrew\nfinalmem\nfinalmemhebrew\n\
finalnun\nfinalnunhebrew\nfinalpe\nfinalpehebrew\nfinaltsadi\nfinaltsadihebrew\n\
firsttonechinese\nfisheye\nfitacyrillic\nfive\nfivearabic\nfivebengali\nfivecircle\n\
fivecircleinversesansserif\nfivedeva\nfiveeighths\nfivegujarati\nfivegurmukhi\n\
fivehackarabic\nfivehangzhou\nfiveideographicparen\nfiveinferior\nfivemonospace\n\
fiveoldstyle\nfiveparen\nfiveperiod\nfivepersian\nfiveroman\nfivesuperior\nfivethai\nfl\n\
flat\nfloorleft\nfloorleftBig\nfloorleftBigg\nfloorleftbig\nfloorleftbigg\nfloorright\n\
floorrightBig\nfloorrightBigg\nfloorrightbig\nfloorrightbigg\nflorin\nfmonospace\n\
fmsquare\nfofanthai\nfofathai\nfollows\nfollowsequal\nfongmanthai\nforall\nfour\n\
fourarabic\nfourbengali\nfourcircle\nfourcircleinversesansserif\nfourdeva\nfourgujarati\n\
fourgurmukhi\nfourhackarabic\nfourhangzhou\nfourideographicparen\nfourinferior\n\
fourmonospace\nfournumeratorbengali\nfouroldstyle\nfourparen\nfourperiod\nfourpersian\n\
fourroman\nfoursuperior\nfourteencircle\nfourteenparen\nfourteenperiod\nfourthai\n\
fourthtonechinese\nfparen\nfraction\nfranc\ng\ngabengali\ngacute\ngadeva\ngafarabic\n\
gaffinalarabic\ngafinitialarabic\ngafmedialarabic\ngagujarati\ngagurmukhi\ngahiragana\n\
gakatakana\ngamma\ngammalatinsmall\ngammasuperior\ngangiacoptic\ngbopomofo\ngbreve\n\
gcaron\ngcedilla\ngcircle\ngcircumflex\ngcommaaccent\ngdot\ngdotaccent\ngecyrillic\n\
gehiragana\ngekatakana\ngeometricallyequal\ngereshaccenthebrew\ngereshhebrew\n\
gereshmuqdamhebrew\ngermandbls\ngershayimaccenthebrew\ngershayimhebrew\ngetamark\n\
ghabengali\nghadarmenian\nghadeva\nghagujarati\nghagurmukhi\nghainarabic\n\
ghainfinalarabic\nghaininitialarabic\nghainmedialarabic\nghemiddlehookcyrillic\n\
ghestrokecyrillic\ngheupturncyrillic\nghhadeva\nghhagurmukhi\nghook\nghzsquare\n\
gihiragana\ngikatakana\ngimarmenian\ngimel\ngimeldagesh\ngimeldageshhebrew\ngimelhebrew\n\
gjecyrillic\nglottalinvertedstroke\nglottalstop\nglottalstopinverted\nglottalstopmod\n\
glottalstopreversed\nglottalstopreversedmod\nglottalstopreversedsuperior\n\
glottalstopstroke\nglottalstopstrokereversed\ngmacron\ngmonospace\ngohiragana\n\
gokatakana\ngparen\ngpasquare\ngradient\ngrave\ngravebelowcmb\ngravecmb\ngravecomb\n\
gravedeva\ngravelowmod\ngravemonospace\ngravetonecmb\ngreater\ngreaterequal\n\
greaterequalorless\ngreatermonospace\ngreatermuch\ngreaterorequivalent\ngreaterorless\n\
greateroverequal\ngreatersmall\ngscript\ngstroke\nguhiragana\nguillemotleft\n\
guillemotright\nguilsinglleft\nguilsinglright\ngukatakana\nguramusquare\ngysquare\nh\n\
haabkhasiancyrillic\nhaaltonearabic\nhabengali\nhadescendercyrillic\nhadeva\nhagujarati\n\
hagurmukhi\nhaharabic\nhahfinalarabic\nhahinitialarabic\nhahiragana\nhahmedialarabic\n\
haitusquare\nhakatakana\nhakatakanahalfwidth\nhalantgurmukhi\nhamzaarabic\n\
hamzadammaarabic\nhamzadammatanarabic\nhamzafathaarabic\nhamzafathatanarabic\n\
hamzalowarabic\nhamzalowkasraarabic\nhamzalowkasratanarabic\nhamzasukunarabic\n\
hangulfiller\nhardsigncyrillic\nharpoonleftbarbup\nharpoonleftdown\nharpoonleftup\n\
harpoonrightbarbup\nharpoonrightdown\nharpoonrightup\nhasquare\nhatafpatah\nhatafpatah16\n\
hatafpatah23\nhatafpatah2f\nhatafpatahhebrew\nhatafpatahnarrowhebrew\n\
hatafpatahquarterhebrew\nhatafpatahwidehebrew\nhatafqamats\nhatafqamats1b\nhatafqamats28\n\
hatafqamats34\nhatafqamatshebrew\nhatafqamatsnarrowhebrew\nhatafqamatsquarterhebrew\n\
hatafqamatswidehebrew\nhatafsegol\nhatafsegol17\nhatafsegol24\nhatafsegol30\n\
hatafsegolhebrew\nhatafsegolnarrowhebrew\nhatafsegolquarterhebrew\nhatafsegolwidehebrew\n\
hatwide\nhatwider\nhatwiderr\nhbar\nhbopomofo\nhbrevebelow\nhcedilla\nhcircle\n\
hcircumflex\nhdieresis\nhdotaccent\nhdotbelow\nhe\nheart\nheart2\nheartsuitblack\n\
heartsuitwhite\nhedagesh\nhedageshhebrew\nhehaltonearabic\nheharabic\nhehebrew\n\
hehfinalaltonearabic\nhehfinalalttwoarabic\nhehfinalarabic\nhehhamzaabovefinalarabic\n\
hehhamzaaboveisolatedarabic\nhehinitialaltonearabic\nhehinitialarabic\nhehiragana\n\
hehmedialaltonearabic\nhehmedialarabic\nheiseierasquare\nhekatakana\nhekatakanahalfwidth\n\
hekutaarusquare\nhenghook\nherutusquare\nhet\nhethebrew\nhhook\nhhooksuperior\n\
hieuhacirclekorean\nhieuhaparenkorean\nhieuhcirclekorean\nhieuhkorean\nhieuhparenkorean\n\
hihiragana\nhikatakana\nhikatakanahalfwidth\nhiriq\nhiriq14\nhiriq21\nhiriq2d\n\
hiriqhebrew\nhiriqnarrowhebrew\nhiriqquarterhebrew\nhiriqwidehebrew\nhlinebelow\n\
hmonospace\nhoarmenian\nhohipthai\nhohiragana\nhokatakana\nhokatakanahalfwidth\nholam\n\
holam19\nholam26\nholam32\nholamhebrew\nholamnarrowhebrew\nholamquarterhebrew\n\
holamwidehebrew\nhonokhukthai\nhookabovecomb\nhookcmb\nhookleftchar\n\
hookpalatalizedbelowcmb\nhookretroflexbelowcmb\nhookrightchar\nhoonsquare\nhoricoptic\n\
horizontalbar\nhorncmb\nhotsprings\nhouse\nhparen\nhsuperior\nhturned\nhuhiragana\n\
huiitosquare\nhukatakana\nhukatakanahalfwidth\nhungarumlaut\nhungarumlautcmb\nhv\nhyphen\n\
hyphen_alt\nhyphenchar\nhypheninferior\nhyphenmonospace\nhyphensmall\nhyphensuperior\n\
hyphentwo\ni\niacute\niacyrillic\nibengali\nibopomofo\nibreve\nicaron\nicircle\n\
icircumflex\nicyrillic\nidblgrave\nideographearthcircle\nideographfirecircle\n\
ideographicallianceparen\nideographiccallparen\nideographiccentrecircle\n\
ideographicclose\nideographiccomma\nideographiccommaleft\nideographiccongratulationparen\n\
ideographiccorrectcircle\nideographicearthparen\nideographicenterpriseparen\n\
ideographicexcellentcircle\nideographicfestivalparen\nideographicfinancialcircle\n\
ideographicfinancialparen\nideographicfireparen\nideographichaveparen\n\
ideographichighcircle\nideographiciterationmark\nideographiclaborcircle\n\
ideographiclaborparen\nideographicleftcircle\nideographiclowcircle\n\
ideographicmedicinecircle\nideographicmetalparen\nideographicmoonparen\n\
ideographicnameparen\nideographicperiod\nideographicprintcircle\nideographicreachparen\n\
ideographicrepresentparen\nideographicresourceparen\nideographicrightcircle\n\
ideographicsecretcircle\nideographicselfparen\nideographicsocietyparen\nideographicspace\n\
ideographicspecialparen\nideographicstockparen\nideographicstudyparen\n\
ideographicsunparen\nideographicsuperviseparen\nideographicwaterparen\n\
ideographicwoodparen\nideographiczero\nideographmetalcircle\nideographmooncircle\n\
ideographnamecircle\nideographsuncircle\nideographwatercircle\nideographwoodcircle\n\
ideva\nidieresis\nidieresisacute\nidieresiscyrillic\nidotbelow\niebrevecyrillic\n\
iecyrillic\nieungacirclekorean\nieungaparenkorean\nieungcirclekorean\nieungkorean\n\
ieungparenkorean\nigrave\nigujarati\nigurmukhi\nihiragana\nihookabove\niibengali\n\
iicyrillic\niideva\niigujarati\niigurmukhi\niimatragurmukhi\niinvertedbreve\n\
iishortcyrillic\niivowelsignbengali\niivowelsigndeva\niivowelsigngujarati\nij\nikatakana\n\
ikatakanahalfwidth\nikorean\nilde\niluyhebrew\nimacron\nimacroncyrillic\n\
imageorapproximatelyequal\nimatragurmukhi\nimonospace\nincrement\ninfinity\niniarmenian\n\
integral\nintegralbottom\nintegralbt\nintegraldisplay\nintegralex\nintegraltext\n\
integraltop\nintegraltp\ninterrobang\ninterrobangdown\nintersection\nintersectiondisplay\n\
intersectionsq\nintersectiontext\nintisquare\ninvbullet\ninvcircle\ninvsmileface\n\
iocyrillic\niogonek\niota\niotadieresis\niotadieresistonos\niotalatin\niotatonos\niparen\n\
irigurmukhi\nismallhiragana\nismallkatakana\nismallkatakanahalfwidth\nissharbengali\n\
istroke\nisuperior\niterationhiragana\niterationkatakana\nitilde\nitildebelow\n\
iubopomofo\niucyrillic\nivowelsignbengali\nivowelsigndeva\nivowelsigngujarati\n\
izhitsacyrillic\nizhitsadblgravecyrillic\nj\njaarmenian\njabengali\njadeva\njagujarati\n\
jagurmukhi\njbopomofo\njcaron\njcircle\njcircumflex\njcrossedtail\njdotlessstroke\n\
jecyrillic\njeemarabic\njeemfinalarabic\njeeminitialarabic\njeemmedialarabic\njeharabic\n\
jehfinalarabic\njhabengali\njhadeva\njhagujarati\njhagurmukhi\njheharmenian\njis\n\
jmonospace\njparen\njsuperior\nk\nkabashkircyrillic\nkabengali\nkacute\nkacyrillic\n\
kadescendercyrillic\nkadeva\nkaf\nkafarabic\nkafdagesh\nkafdageshhebrew\nkaffinalarabic\n\
kafhebrew\nkafinitialarabic\nkafmedialarabic\nkafrafehebrew\nkagujarati\nkagurmukhi\n\
kahiragana\nkahookcyrillic\nkakatakana\nkakatakanahalfwidth\nkappa\nkappasymbolgreek\n\
kapyeounmieumkorean\nkapyeounphieuphkorean\nkapyeounpieupkorean\n\
kapyeounssangpieupkorean\nkaroriisquare\nkashidaautoarabic\n\
kashidaautonosidebearingarabic\nkasmallkatakana\nkasquare\nkasraarabic\nkasratanarabic\n\
kastrokecyrillic\nkatahiraprolongmarkhalfwidth\nkaverticalstrokecyrillic\nkbopomofo\n\
kcalsquare\nkcaron\nkcedilla\nkcircle\nkcommaaccent\nkdotbelow\nkeharmenian\nkehiragana\n\
kekatakana\nkekatakanahalfwidth\nkenarmenian\nkesmallkatakana\nkgreenlandic\nkhabengali\n\
khacyrillic\nkhadeva\nkhagujarati\nkhagurmukhi\nkhaharabic\nkhahfinalarabic\n\
khahinitialarabic\nkhahmedialarabic\nkheicoptic\nkhhadeva\nkhhagurmukhi\n\
khieukhacirclekorean\nkhieukhaparenkorean\nkhieukhcirclekorean\nkhieukhkorean\n\
khieukhparenkorean\nkhokhaithai\nkhokhonthai\nkhokhuatthai\nkhokhwaithai\nkhomutthai\n\
khook\nkhorakhangthai\nkhzsquare\nkihiragana\nkikatakana\nkikatakanahalfwidth\n\
kiroguramusquare\nkiromeetorusquare\nkirosquare\nkiyeokacirclekorean\nkiyeokaparenkorean\n\
kiyeokcirclekorean\nkiyeokkorean\nkiyeokparenkorean\nkiyeoksioskorean\nkjecyrillic\n\
klinebelow\nklsquare\nkmcubedsquare\nkmonospace\nkmsquaredsquare\nkohiragana\nkohmsquare\n\
kokaithai\nkokatakana\nkokatakanahalfwidth\nkooposquare\nkoppacyrillic\n\
koreanstandardsymbol\nkoroniscmb\nkparen\nkpasquare\nksicyrillic\nktsquare\nkturned\n\
kuhiragana\nkukatakana\nkukatakanahalfwidth\nkvsquare\nkwsquare\nl\nlabengali\nlacute\n\
ladeva\nlagujarati\nlagurmukhi\nlakkhangyaothai\nlamaleffinalarabic\n\
lamalefhamzaabovefinalarabic\nlamalefhamzaaboveisolatedarabic\n\
lamalefhamzabelowfinalarabic\nlamalefhamzabelowisolatedarabic\nlamalefisolatedarabic\n\
lamalefmaddaabovefinalarabic\nlamalefmaddaaboveisolatedarabic\nlamarabic\nlambda\n\
lambdastroke\nlamed\nlameddagesh\nlameddageshhebrew\nlamedhebrew\nlamedholam\n\
lamedholamdagesh\nlamedholamdageshhebrew\nlamedholamhebrew\nlamfinalarabic\n\
lamhahinitialarabic\nlaminitialarabic\nlamjeeminitialarabic\nlamkhahinitialarabic\n\
lamlamhehisolatedarabic\nlammedialarabic\nlammeemhahinitialarabic\nlammeeminitialarabic\n\
lammeemjeeminitialarabic\nlammeemkhahinitialarabic\nlargecircle\nlatticetop\nlbar\nlbelt\n\
lbopomofo\nlcaron\nlcedilla\nlcircle\nlcircumflexbelow\nlcommaaccent\nldot\nldotaccent\n\
ldotbelow\nldotbelowmacron\nleftangleabovecmb\nlefttackbelowcmb\nless\nlessequal\n\
lessequalorgreater\nlessmonospace\nlessmuch\nlessorequivalent\nlessorgreater\n\
lessoverequal\nlesssmall\nlezh\nlfblock\nlhookretroflex\nlira\nliwnarmenian\nlj\n\
ljecyrillic\nll\nlladeva\nllagujarati\nllinebelow\nllladeva\nllvocalicbengali\n\
llvocalicdeva\nllvocalicvowelsignbengali\nllvocalicvowelsigndeva\nlmiddletilde\n\
lmonospace\nlmsquare\nlochulathai\nlogicaland\nlogicalanddisplay\nlogicalandtext\n\
logicalnot\nlogicalnotreversed\nlogicalor\nlogicalordisplay\nlogicalortext\nlolingthai\n\
longs\nlowlinecenterline\nlowlinecmb\nlowlinedashed\nlozenge\nlparen\nlscript\nlslash\n\
lsquare\nlsuperior\nltshade\nluthai\nlvocalicbengali\nlvocalicdeva\n\
lvocalicvowelsignbengali\nlvocalicvowelsigndeva\nlxsquare\nm\nmabengali\nmacron\n\
macronbelowcmb\nmacroncmb\nmacronlowmod\nmacronmonospace\nmacute\nmadeva\nmagujarati\n\
magurmukhi\nmahapakhhebrew\nmahapakhlefthebrew\nmahiragana\nmaichattawalowleftthai\n\
maichattawalowrightthai\nmaichattawathai\nmaichattawaupperleftthai\nmaieklowleftthai\n\
maieklowrightthai\nmaiekthai\nmaiekupperleftthai\nmaihanakatleftthai\nmaihanakatthai\n\
maitaikhuleftthai\nmaitaikhuthai\nmaitholowleftthai\nmaitholowrightthai\nmaithothai\n\
maithoupperleftthai\nmaitrilowleftthai\nmaitrilowrightthai\nmaitrithai\n\
maitriupperleftthai\nmaiyamokthai\nmakatakana\nmakatakanahalfwidth\nmale\nmansyonsquare\n\
maqafhebrew\nmars\nmasoracirclehebrew\nmasquare\nmbopomofo\nmbsquare\nmcircle\n\
mcubedsquare\nmdotaccent\nmdotbelow\nmeemarabic\nmeemfinalarabic\nmeeminitialarabic\n\
meemmedialarabic\nmeemmeeminitialarabic\nmeemmeemisolatedarabic\nmeetorusquare\n\
mehiragana\nmeizierasquare\nmekatakana\nmekatakanahalfwidth\nmem\nmemdagesh\n\
memdageshhebrew\nmemhebrew\nmenarmenian\nmerkhahebrew\nmerkhakefulahebrew\n\
merkhakefulalefthebrew\nmerkhalefthebrew\nmhook\nmhzsquare\nmiddledotkatakanahalfwidth\n\
middot\nmieumacirclekorean\nmieumaparenkorean\nmieumcirclekorean\nmieumkorean\n\
mieumpansioskorean\nmieumparenkorean\nmieumpieupkorean\nmieumsioskorean\nmihiragana\n\
mikatakana\nmikatakanahalfwidth\nminus\nminusbelowcmb\nminuscircle\nminusmod\nminusplus\n\
minute\nmiribaarusquare\nmirisquare\nmlonglegturned\nmlsquare\nmmcubedsquare\nmmonospace\n\
mmsquaredsquare\nmohiragana\nmohmsquare\nmokatakana\nmokatakanahalfwidth\nmolsquare\n\
momathai\nmoverssquare\nmoverssquaredsquare\nmparen\nmpasquare\nmssquare\nmsuperior\n\
mturned\nmu\nmu1\nmuasquare\nmuchgreater\nmuchless\nmufsquare\nmugreek\nmugsquare\n\
muhiragana\nmukatakana\nmukatakanahalfwidth\nmulsquare\nmultiply\nmumsquare\nmunahhebrew\n\
munahlefthebrew\nmusicalnote\nmusicalnotedbl\nmusicflatsign\nmusicsharpsign\nmussquare\n\
muvsquare\nmuwsquare\nmvmegasquare\nmvsquare\nmwmegasquare\nmwsquare\nn\nnabengali\n\
nabla\nnacute\nnadeva\nnagujarati\nnagurmukhi\nnahiragana\nnakatakana\n\
nakatakanahalfwidth\nnapostrophe\nnasquare\nnatural\nnbopomofo\nnbspace\nncaron\n\
ncedilla\nncircle\nncircumflexbelow\nncommaaccent\nndotaccent\nndotbelow\nnegationslash\n\
nehiragana\nnekatakana\nnekatakanahalfwidth\nnewsheqelsign\nnfsquare\nng\nngabengali\n\
ngadeva\nngagujarati\nngagurmukhi\nngonguthai\nnhiragana\nnhookleft\nnhookretroflex\n\
nieunacirclekorean\nnieunaparenkorean\nnieuncieuckorean\nnieuncirclekorean\n\
nieunhieuhkorean\nnieunkorean\nnieunpansioskorean\nnieunparenkorean\nnieunsioskorean\n\
nieuntikeutkorean\nnihiragana\nnikatakana\nnikatakanahalfwidth\nnikhahitleftthai\n\
nikhahitthai\nnine\nninearabic\nninebengali\nninecircle\nninecircleinversesansserif\n\
ninedeva\nninegujarati\nninegurmukhi\nninehackarabic\nninehangzhou\nnineideographicparen\n\
nineinferior\nninemonospace\nnineoldstyle\nnineparen\nnineperiod\nninepersian\nnineroman\n\
ninesuperior\nnineteencircle\nnineteenparen\nnineteenperiod\nninethai\nnj\nnjecyrillic\n\
nkatakana\nnkatakanahalfwidth\nnlegrightlong\nnlinebelow\nnmonospace\nnmsquare\n\
nnabengali\nnnadeva\nnnagujarati\nnnagurmukhi\nnnnadeva\nnohiragana\nnokatakana\n\
nokatakanahalfwidth\nnonbreakingspace\nnonenthai\nnonuthai\nnoonarabic\nnoonfinalarabic\n\
noonghunnaarabic\nnoonghunnafinalarabic\nnoonhehinitialarabic\nnooninitialarabic\n\
noonjeeminitialarabic\nnoonjeemisolatedarabic\nnoonmedialarabic\nnoonmeeminitialarabic\n\
noonmeemisolatedarabic\nnoonnoonfinalarabic\nnotcontains\nnotelement\nnotelementof\n\
notequal\nnotgreater\nnotgreaternorequal\nnotgreaternorless\nnotidentical\nnotless\n\
notlessnorequal\nnotparallel\nnotprecedes\nnotsubset\nnotsucceeds\nnotsuperset\n\
nowarmenian\nnparen\nnssquare\nnsuperior\nntilde\nnu\nnuhiragana\nnukatakana\n\
nukatakanahalfwidth\nnuktabengali\nnuktadeva\nnuktagujarati\nnuktagurmukhi\nnumbersign\n\
numbersignmonospace\nnumbersignsmall\nnumeralsigngreek\nnumeralsignlowergreek\nnumero\n\
nun\nnundagesh\nnundageshhebrew\nnunhebrew\nnvsquare\nnwsquare\nnyabengali\nnyadeva\n\
nyagujarati\nnyagurmukhi\no\noacute\noangthai\nobarred\nobarredcyrillic\n\
obarreddieresiscyrillic\nobengali\nobopomofo\nobreve\nocandradeva\nocandragujarati\n\
ocandravowelsigndeva\nocandravowelsigngujarati\nocaron\nocircle\nocircumflex\n\
ocircumflexacute\nocircumflexdotbelow\nocircumflexgrave\nocircumflexhookabove\n\
ocircumflextilde\nocyrillic\nodblacute\nodblgrave\nodeva\nodieresis\nodieresiscyrillic\n\
odotbelow\noe\noekorean\nogonek\nogonekcmb\nograve\nogujarati\noharmenian\nohiragana\n\
ohookabove\nohorn\nohornacute\nohorndotbelow\nohorngrave\nohornhookabove\nohorntilde\n\
ohungarumlaut\noi\noinvertedbreve\nokatakana\nokatakanahalfwidth\nokorean\nolehebrew\n\
omacron\nomacronacute\nomacrongrave\nomdeva\nomega\nomega1\nomegacyrillic\n\
omegalatinclosed\nomegaroundcyrillic\nomegatitlocyrillic\nomegatonos\nomgujarati\n\
omicron\nomicrontonos\nomonospace\none\nonearabic\nonebengali\nonecircle\n\
onecircleinversesansserif\nonedeva\nonedotenleader\noneeighth\nonefitted\nonegujarati\n\
onegurmukhi\nonehackarabic\nonehalf\nonehangzhou\noneideographicparen\noneinferior\n\
onemonospace\nonenumeratorbengali\noneoldstyle\noneparen\noneperiod\nonepersian\n\
onequarter\noneroman\nonesuperior\nonethai\nonethird\noogonek\noogonekmacron\noogurmukhi\n\
oomatragurmukhi\noopen\noparen\nopenbullet\noption\nordfeminine\nordmasculine\n\
orthogonal\noshortdeva\noshortvowelsigndeva\noslash\noslashacute\nosmallhiragana\n\
osmallkatakana\nosmallkatakanahalfwidth\nostrokeacute\nosuperior\notcyrillic\notilde\n\
otildeacute\notildedieresis\noubopomofo\noverline\noverlinecenterline\noverlinecmb\n\
overlinedashed\noverlinedblwavy\noverlinewavy\noverscore\novowelsignbengali\n\
ovowelsigndeva\novowelsigngujarati\nowner\np\npaampssquare\npaasentosquare\npabengali\n\
pacute\npadeva\npagedown\npageup\npagujarati\npagurmukhi\npahiragana\npaiyannoithai\n\
pakatakana\npalatalizationcyrilliccmb\npalochkacyrillic\npansioskorean\nparagraph\n\
parallel\nparenleft\nparenleftBig\nparenleftBigg\nparenleftaltonearabic\nparenleftbig\n\
parenleftbigg\nparenleftbt\nparenleftex\nparenleftex2\nparenleftinferior\n\
parenleftmonospace\nparenleftsmall\nparenleftsuperior\nparenlefttp\nparenleftvertical\n\
parenright\nparenrightBig\nparenrightBigg\nparenrightaltonearabic\nparenrightbig\n\
parenrightbigg\nparenrightbt\nparenrightex\nparenrightex2\nparenrightinferior\n\
parenrightmonospace\nparenrightsmall\nparenrightsuperior\nparenrighttp\n\
parenrightvertical\npartialdiff\npaseqhebrew\npashtahebrew\npasquare\npatah\npatah11\n\
patah1d\npatah2a\npatahhebrew\npatahnarrowhebrew\npatahquarterhebrew\npatahwidehebrew\n\
pazerhebrew\npbopomofo\npcircle\npdotaccent\npe\npecyrillic\npedagesh\npedageshhebrew\n\
peezisquare\npefinaldageshhebrew\npeharabic\npeharmenian\npehebrew\npehfinalarabic\n\
pehinitialarabic\npehiragana\npehmedialarabic\npekatakana\npemiddlehookcyrillic\n\
perafehebrew\npercent\npercentarabic\npercentmonospace\npercentsmall\nperiod\n\
periodarmenian\nperiodcentered\nperiodhalfwidth\nperiodinferior\nperiodmonospace\n\
periodsmall\nperiodsuperior\nperispomenigreekcmb\nperpendicular\npertenthousand\n\
perthousand\npeseta\npfsquare\nphabengali\nphadeva\nphagujarati\nphagurmukhi\nphi\nphi2\n\
phieuphacirclekorean\nphieuphaparenkorean\nphieuphcirclekorean\nphieuphkorean\n\
phieuphparenkorean\nphilatin\nphinthuthai\nphisymbolgreek\nphook\nphophanthai\n\
phophungthai\nphosamphaothai\npi\npi1\npieupacirclekorean\npieupaparenkorean\n\
pieupcieuckorean\npieupcirclekorean\npieupkiyeokkorean\npieupkorean\npieupparenkorean\n\
pieupsioskiyeokkorean\npieupsioskorean\npieupsiostikeutkorean\npieupthieuthkorean\n\
pieuptikeutkorean\npihiragana\npikatakana\npisymbolgreek\npiwrarmenian\nplus\n\
plusbelowcmb\npluscircle\nplusminus\nplusmod\nplusmonospace\nplussmall\nplussuperior\n\
pmonospace\npmsquare\npohiragana\npointingindexdownwhite\npointingindexleftwhite\n\
pointingindexrightwhite\npointingindexupwhite\npokatakana\npoplathai\npostalmark\n\
postalmarkface\npparen\nprecedes\nprecedesequal\nprescription\nprime\nprimemod\n\
primereversed\nproduct\nproductdisplay\nproducttext\nprojective\nprolongedkana\n\
propellor\npropersubset\npropersuperset\nproportion\nproportional\npsi\npsicyrillic\n\
psilipneumatacyrilliccmb\npssquare\npuhiragana\npukatakana\npunctdash\npvsquare\n\
pwsquare\nq\nqadeva\nqadmahebrew\nqafarabic\nqaffinalarabic\nqafinitialarabic\n\
qafmedialarabic\nqamats\nqamats10\nqamats1a\nqamats1c\nqamats27\nqamats29\nqamats33\n\
qamatsde\nqamatshebrew\nqamatsnarrowhebrew\nqamatsqatanhebrew\nqamatsqatannarrowhebrew\n\
qamatsqatanquarterhebrew\nqamatsqatanwidehebrew\nqamatsquarterhebrew\nqamatswidehebrew\n\
qarneyparahebrew\nqbopomofo\nqcircle\nqhook\nqmonospace\nqof\nqofdagesh\nqofdageshhebrew\n\
qofhatafpatah\nqofhatafpatahhebrew\nqofhatafsegol\nqofhatafsegolhebrew\nqofhebrew\n\
qofhiriq\nqofhiriqhebrew\nqofholam\nqofholamhebrew\nqofpatah\nqofpatahhebrew\nqofqamats\n\
qofqamatshebrew\nqofqubuts\nqofqubutshebrew\nqofsegol\nqofsegolhebrew\nqofsheva\n\
qofshevahebrew\nqoftsere\nqoftserehebrew\nqparen\nquarternote\nqubuts\nqubuts18\n\
qubuts25\nqubuts31\nqubutshebrew\nqubutsnarrowhebrew\nqubutsquarterhebrew\n\
qubutswidehebrew\nquestion\nquestionarabic\nquestionarmenian\nquestiondown\n\
questiondownsmall\nquestiongreek\nquestionmonospace\nquestionsmall\nquotedbl\n\
quotedblbase\nquotedblleft\nquotedblmonospace\nquotedblprime\nquotedblprimereversed\n\
quotedblright\nquoteleft\nquoteleftreversed\nquotereversed\nquoteright\nquoterightn\n\
quotesinglbase\nquotesingle\nquotesinglemonospace\nr\nraarmenian\nrabengali\nracute\n\
radeva\nradical\nradicalBig\nradicalBigg\nradicalbig\nradicalbigg\nradicalbt\nradicalex\n\
radoverssquare\nradoverssquaredsquare\nradsquare\nrafe\nrafehebrew\nragujarati\n\
ragurmukhi\nrahiragana\nrakatakana\nrakatakanahalfwidth\nralowerdiagonalbengali\n\
ramiddlediagonalbengali\nramshorn\nrangedash\nratio\nrbopomofo\nrcaron\nrcedilla\n\
rcircle\nrcommaaccent\nrdblgrave\nrdotaccent\nrdotbelow\nrdotbelowmacron\nreferencemark\n\
reflexsubset\nreflexsuperset\nregistered\nregistersans\nregisterserif\nreharabic\n\
reharmenian\nrehfinalarabic\nrehiragana\nrehyehaleflamarabic\nrekatakana\n\
rekatakanahalfwidth\nresh\nreshdageshhebrew\nreshhatafpatah\nreshhatafpatahhebrew\n\
reshhatafsegol\nreshhatafsegolhebrew\nreshhebrew\nreshhiriq\nreshhiriqhebrew\nreshholam\n\
reshholamhebrew\nreshpatah\nreshpatahhebrew\nreshqamats\nreshqamatshebrew\nreshqubuts\n\
reshqubutshebrew\nreshsegol\nreshsegolhebrew\nreshsheva\nreshshevahebrew\nreshtsere\n\
reshtserehebrew\nreversedtilde\nreviahebrew\nreviamugrashhebrew\nrevlogicalnot\n\
rfishhook\nrfishhookreversed\nrhabengali\nrhadeva\nrho\nrho1\nrhook\nrhookturned\n\
rhookturnedsuperior\nrhosymbolgreek\nrhotichookmod\nrieulacirclekorean\n\
rieulaparenkorean\nrieulcirclekorean\nrieulhieuhkorean\nrieulkiyeokkorean\n\
rieulkiyeoksioskorean\nrieulkorean\nrieulmieumkorean\nrieulpansioskorean\n\
rieulparenkorean\nrieulphieuphkorean\nrieulpieupkorean\nrieulpieupsioskorean\n\
rieulsioskorean\nrieulthieuthkorean\nrieultikeutkorean\nrieulyeorinhieuhkorean\n\
rightangle\nrighttackbelowcmb\nrighttriangle\nrihiragana\nrikatakana\n\
rikatakanahalfwidth\nring\nringbelowcmb\nringcmb\nringhalfleft\nringhalfleftarmenian\n\
ringhalfleftbelowcmb\nringhalfleftcentered\nringhalfright\nringhalfrightbelowcmb\n\
ringhalfrightcentered\nrinvertedbreve\nrittorusquare\nrlinebelow\nrlongleg\n\
rlonglegturned\nrmonospace\nrohiragana\nrokatakana\nrokatakanahalfwidth\nroruathai\n\
rparen\nrrabengali\nrradeva\nrragurmukhi\nrreharabic\nrrehfinalarabic\nrrvocalicbengali\n\
rrvocalicdeva\nrrvocalicgujarati\nrrvocalicvowelsignbengali\nrrvocalicvowelsigndeva\n\
rrvocalicvowelsigngujarati\nrsuperior\nrtblock\nrturned\nrturnedsuperior\nruhiragana\n\
rukatakana\nrukatakanahalfwidth\nrupeemarkbengali\nrupeesignbengali\nrupiah\nruthai\n\
rvocalicbengali\nrvocalicdeva\nrvocalicgujarati\nrvocalicvowelsignbengali\n\
rvocalicvowelsigndeva\nrvocalicvowelsigngujarati\ns\nsabengali\nsacute\nsacutedotaccent\n\
sadarabic\nsadeva\nsadfinalarabic\nsadinitialarabic\nsadmedialarabic\nsagujarati\n\
sagurmukhi\nsahiragana\nsakatakana\nsakatakanahalfwidth\nsallallahoualayhewasallamarabic\n\
samekh\nsamekhdagesh\nsamekhdageshhebrew\nsamekhhebrew\nsaraaathai\nsaraaethai\n\
saraaimaimalaithai\nsaraaimaimuanthai\nsaraamthai\nsaraathai\nsaraethai\nsaraiileftthai\n\
saraiithai\nsaraileftthai\nsaraithai\nsaraothai\nsaraueeleftthai\nsaraueethai\n\
saraueleftthai\nsarauethai\nsarauthai\nsarauuthai\nsbopomofo\nscaron\nscarondotaccent\n\
scedilla\nschwa\nschwacyrillic\nschwadieresiscyrillic\nschwahook\nscircle\nscircumflex\n\
scommaaccent\nsdotaccent\nsdotbelow\nsdotbelowdotaccent\nseagullbelowcmb\nsecond\n\
secondtonechinese\nsection\nseenarabic\nseenfinalarabic\nseeninitialarabic\n\
seenmedialarabic\nsegol\nsegol13\nsegol1f\nsegol2c\nsegolhebrew\nsegolnarrowhebrew\n\
segolquarterhebrew\nsegoltahebrew\nsegolwidehebrew\nseharmenian\nsehiragana\nsekatakana\n\
sekatakanahalfwidth\nsemicolon\nsemicolonarabic\nsemicolonmonospace\nsemicolonsmall\n\
semivoicedmarkkana\nsemivoicedmarkkanahalfwidth\nsentisquare\nsentosquare\nseven\n\
sevenarabic\nsevenbengali\nsevencircle\nsevencircleinversesansserif\nsevendeva\n\
seveneighths\nsevengujarati\nsevengurmukhi\nsevenhackarabic\nsevenhangzhou\n\
sevenideographicparen\nseveninferior\nsevenmonospace\nsevenoldstyle\nsevenparen\n\
sevenperiod\nsevenpersian\nsevenroman\nsevensuperior\nseventeencircle\nseventeenparen\n\
seventeenperiod\nseventhai\nsfthyphen\nshaarmenian\nshabengali\nshacyrillic\n\
shaddaarabic\nshaddadammaarabic\nshaddadammatanarabic\nshaddafathaarabic\n\
shaddafathatanarabic\nshaddakasraarabic\nshaddakasratanarabic\nshade\nshadedark\n\
shadelight\nshademedium\nshadeva\nshagujarati\nshagurmukhi\nshalshelethebrew\nsharp\n\
shbopomofo\nshchacyrillic\nsheenarabic\nsheenfinalarabic\nsheeninitialarabic\n\
sheenmedialarabic\nsheicoptic\nsheqel\nsheqelhebrew\nsheva\nsheva115\nsheva15\nsheva22\n\
sheva2e\nshevahebrew\nshevanarrowhebrew\nshevaquarterhebrew\nshevawidehebrew\n\
shhacyrillic\nshimacoptic\nshin\nshindagesh\nshindageshhebrew\nshindageshshindot\n\
shindageshshindothebrew\nshindageshsindot\nshindageshsindothebrew\nshindothebrew\n\
shinhebrew\nshinshindot\nshinshindothebrew\nshinsindot\nshinsindothebrew\nshook\nsigma\n\
sigma1\nsigmafinal\nsigmalunatesymbolgreek\nsihiragana\nsikatakana\nsikatakanahalfwidth\n\
siluqhebrew\nsiluqlefthebrew\nsimilar\nsimilarequal\nsindothebrew\nsiosacirclekorean\n\
siosaparenkorean\nsioscieuckorean\nsioscirclekorean\nsioskiyeokkorean\nsioskorean\n\
siosnieunkorean\nsiosparenkorean\nsiospieupkorean\nsiostikeutkorean\nsix\nsixarabic\n\
sixbengali\nsixcircle\nsixcircleinversesansserif\nsixdeva\nsixgujarati\nsixgurmukhi\n\
sixhackarabic\nsixhangzhou\nsixideographicparen\nsixinferior\nsixmonospace\nsixoldstyle\n\
sixparen\nsixperiod\nsixpersian\nsixroman\nsixsuperior\nsixteencircle\n\
sixteencurrencydenominatorbengali\nsixteenparen\nsixteenperiod\nsixthai\nslash\nslashBig\n\
slashBigg\nslashbig\nslashbigg\nslashmonospace\nslong\nslongdotaccent\nslurabove\n\
slurbelow\nsmileface\nsmonospace\nsofpasuqhebrew\nsofthyphen\nsoftsigncyrillic\n\
sohiragana\nsokatakana\nsokatakanahalfwidth\nsoliduslongoverlaycmb\n\
solidusshortoverlaycmb\nsorusithai\nsosalathai\nsosothai\nsosuathai\nspace\n\
spacehackarabic\nspade\nspadesuitblack\nspadesuitwhite\nsparen\nsquarebelowcmb\nsquarecc\n\
squarecm\nsquarediagonalcrosshatchfill\nsquarehorizontalfill\nsquarekg\nsquarekm\n\
squarekmcapital\nsquareln\nsquarelog\nsquaremg\nsquaremil\nsquaremm\nsquaremsquared\n\
squareorthogonalcrosshatchfill\nsquareupperlefttolowerrightfill\n\
squareupperrighttolowerleftfill\nsquareverticalfill\nsquarewhitewithsmallblack\nsrsquare\n\
ssabengali\nssadeva\nssagujarati\nssangcieuckorean\nssanghieuhkorean\nssangieungkorean\n\
ssangkiyeokkorean\nssangnieunkorean\nssangpieupkorean\nssangsioskorean\n\
ssangtikeutkorean\nssuperior\nstar\nsterling\nsterlingmonospace\nstrokelongoverlaycmb\n\
strokeshortoverlaycmb\nsubset\nsubsetnotequal\nsubsetorequal\nsubsetsqequal\nsucceeds\n\
suchthat\nsuhiragana\nsukatakana\nsukatakanahalfwidth\nsukunarabic\nsummation\n\
summationdisplay\nsummationtext\nsun\nsuperset\nsupersetnotequal\nsupersetorequal\n\
supersetsqequal\nsvsquare\nsyouwaerasquare\nt\ntabengali\ntackdown\ntackleft\ntadeva\n\
tagujarati\ntagurmukhi\ntaharabic\ntahfinalarabic\ntahinitialarabic\ntahiragana\n\
tahmedialarabic\ntaisyouerasquare\ntakatakana\ntakatakanahalfwidth\ntatweelarabic\ntau\n\
tav\ntavdages\ntavdagesh\ntavdageshhebrew\ntavhebrew\ntbar\ntbopomofo\ntcaron\ntccurl\n\
tcedilla\ntcheharabic\ntchehfinalarabic\ntchehinitialarabic\ntchehmedialarabic\n\
tchehmeeminitialarabic\ntcircle\ntcircumflexbelow\ntcommaaccent\ntdieresis\ntdotaccent\n\
tdotbelow\ntecyrillic\ntedescendercyrillic\nteharabic\ntehfinalarabic\n\
tehhahinitialarabic\ntehhahisolatedarabic\ntehinitialarabic\ntehiragana\n\
tehjeeminitialarabic\ntehjeemisolatedarabic\ntehmarbutaarabic\ntehmarbutafinalarabic\n\
tehmedialarabic\ntehmeeminitialarabic\ntehmeemisolatedarabic\ntehnoonfinalarabic\n\
tekatakana\ntekatakanahalfwidth\ntelephone\ntelephoneblack\ntelishagedolahebrew\n\
telishaqetanahebrew\ntencircle\ntenideographicparen\ntenparen\ntenperiod\ntenroman\ntesh\n\
tet\ntetdagesh\ntetdageshhebrew\ntethebrew\ntetsecyrillic\ntevirhebrew\ntevirlefthebrew\n\
thabengali\nthadeva\nthagujarati\nthagurmukhi\nthalarabic\nthalfinalarabic\n\
thanthakhatlowleftthai\nthanthakhatlowrightthai\nthanthakhatthai\n\
thanthakhatupperleftthai\ntheharabic\nthehfinalarabic\nthehinitialarabic\n\
thehmedialarabic\nthereexists\ntherefore\ntheta\ntheta1\nthetasymbolgreek\n\
thieuthacirclekorean\nthieuthaparenkorean\nthieuthcirclekorean\nthieuthkorean\n\
thieuthparenkorean\nthirteencircle\nthirteenparen\nthirteenperiod\nthonangmonthothai\n\
thook\nthophuthaothai\nthorn\nthothahanthai\nthothanthai\nthothongthai\nthothungthai\n\
thousandcyrillic\nthousandsseparatorarabic\nthousandsseparatorpersian\nthree\n\
threearabic\nthreebengali\nthreecircle\nthreecircleinversesansserif\nthreedeva\n\
threeeighths\nthreegujarati\nthreegurmukhi\nthreehackarabic\nthreehangzhou\n\
threeideographicparen\nthreeinferior\nthreemonospace\nthreenumeratorbengali\n\
threeoldstyle\nthreeparen\nthreeperiod\nthreepersian\nthreequarters\nthreequartersemdash\n\
threeroman\nthreesuperior\nthreethai\nthzsquare\ntie\ntihiragana\ntikatakana\n\
tikatakanahalfwidth\ntikeutacirclekorean\ntikeutaparenkorean\ntikeutcirclekorean\n\
tikeutkorean\ntikeutparenkorean\ntilde\ntildebelowcmb\ntildecmb\ntildecomb\n\
tildedoublecmb\ntildeoperator\ntildeoverlaycmb\ntildeverticalcmb\ntildewide\ntildewider\n\
tildewiderr\ntimescircle\ntipehahebrew\ntipehalefthebrew\ntippigurmukhi\n\
titlocyrilliccmb\ntiwnarmenian\ntlinebelow\ntmonospace\ntoarmenian\ntohiragana\n\
tokatakana\ntokatakanahalfwidth\ntonebarextrahighmod\ntonebarextralowmod\ntonebarhighmod\n\
tonebarlowmod\ntonebarmidmod\ntonefive\ntonesix\ntonetwo\ntonos\ntonsquare\ntopatakthai\n\
tortoiseshellbracketleft\ntortoiseshellbracketleftsmall\n\
tortoiseshellbracketleftvertical\ntortoiseshellbracketright\n\
tortoiseshellbracketrightsmall\ntortoiseshellbracketrightvertical\ntotaothai\n\
tpalatalhook\ntparen\ntrademark\ntrademarksans\ntrademarkserif\ntretroflexhook\ntriagdn\n\
triaglf\ntriagrt\ntriagup\ntriangle\ntriangleinv\ntriangleleft\ntriangleright\nts\ntsadi\n\
tsadidagesh\ntsadidageshhebrew\ntsadihebrew\ntsecyrillic\ntsere\ntsere12\ntsere1e\n\
tsere2b\ntserehebrew\ntserenarrowhebrew\ntserequarterhebrew\ntserewidehebrew\n\
tshecyrillic\ntsuperior\nttabengali\nttadeva\nttagujarati\nttagurmukhi\ntteharabic\n\
ttehfinalarabic\nttehinitialarabic\nttehmedialarabic\ntthabengali\ntthadeva\n\
tthagujarati\ntthagurmukhi\ntturned\ntuhiragana\ntukatakana\ntukatakanahalfwidth\n\
turnstileleft\nturnstileright\ntusmallhiragana\ntusmallkatakana\n\
tusmallkatakanahalfwidth\ntwelvecircle\ntwelveparen\ntwelveperiod\ntwelveroman\n\
twelveudash\ntwentycircle\ntwentyhangzhou\ntwentyparen\ntwentyperiod\ntwo\ntwoarabic\n\
twobengali\ntwocircle\ntwocircleinversesansserif\ntwodeva\ntwodotenleader\ntwodotleader\n\
twodotleadervertical\ntwogujarati\ntwogurmukhi\ntwohackarabic\ntwohangzhou\n\
twoideographicparen\ntwoinferior\ntwomonospace\ntwonumeratorbengali\ntwooldstyle\n\
twoparen\ntwoperiod\ntwopersian\ntworoman\ntwostroke\ntwosuperior\ntwothai\ntwothirds\nu\n\
uacute\nubar\nubengali\nubopomofo\nubreve\nucaron\nucircle\nucircumflex\n\
ucircumflexbelow\nucyrillic\nudattadeva\nudblacute\nudblgrave\nudeva\nudieresis\n\
udieresisacute\nudieresisbelow\nudieresiscaron\nudieresiscyrillic\nudieresisgrave\n\
udieresismacron\nudotbelow\nugrave\nugujarati\nugurmukhi\nuhiragana\nuhookabove\nuhorn\n\
uhornacute\nuhorndotbelow\nuhorngrave\nuhornhookabove\nuhorntilde\nuhungarumlaut\n\
uhungarumlautcyrillic\nuinvertedbreve\nukatakana\nukatakanahalfwidth\nukcyrillic\n\
ukorean\numacron\numacroncyrillic\numacrondieresis\numatragurmukhi\numonospace\n\
underscore\nunderscoredbl\nunderscoremonospace\nunderscorevertical\nunderscorewavy\n\
union\nuniondisplay\nunionmulti\nunionmultidisplay\nunionmultitext\nunionsq\n\
unionsqdisplay\nunionsqtext\nuniontext\nuniversal\nuogonek\nuparen\nupblock\n\
upperdothebrew\nupsilon\nupsilondieresis\nupsilondieresistonos\nupsilonlatin\n\
upsilontonos\nuptackbelowcmb\nuptackmod\nuragurmukhi\nuring\nushortcyrillic\n\
usmallhiragana\nusmallkatakana\nusmallkatakanahalfwidth\nustraightcyrillic\n\
ustraightstrokecyrillic\nutilde\nutildeacute\nutildebelow\nuubengali\nuudeva\nuugujarati\n\
uugurmukhi\nuumatragurmukhi\nuuvowelsignbengali\nuuvowelsigndeva\nuuvowelsigngujarati\n\
uvowelsignbengali\nuvowelsigndeva\nuvowelsigngujarati\nv\nvadeva\nvagujarati\nvagurmukhi\n\
vakatakana\nvav\nvavdagesh\nvavdagesh65\nvavdageshhebrew\nvavhebrew\nvavholam\n\
vavholamhebrew\nvavvavhebrew\nvavyodhebrew\nvcircle\nvdotbelow\nvector\nvecyrillic\n\
veharabic\nvehfinalarabic\nvehinitialarabic\nvehmedialarabic\nvekatakana\nvenus\n\
verticalbar\nverticallineabovecmb\nverticallinebelowcmb\nverticallinelowmod\n\
verticallinemod\nvewarmenian\nvhook\nvikatakana\nviramabengali\nviramadeva\n\
viramagujarati\nvisargabengali\nvisargadeva\nvisargagujarati\nvisiblespace\nvisualspace\n\
vmonospace\nvoarmenian\nvoicediterationhiragana\nvoicediterationkatakana\nvoicedmarkkana\n\
voicedmarkkanahalfwidth\nvokatakana\nvparen\nvtilde\nvturned\nvuhiragana\nvukatakana\nw\n\
wacute\nwaekorean\nwahiragana\nwakatakana\nwakatakanahalfwidth\nwakorean\n\
wasmallhiragana\nwasmallkatakana\nwattosquare\nwavedash\nwavyunderscorevertical\n\
wawarabic\nwawfinalarabic\nwawhamzaabovearabic\nwawhamzaabovefinalarabic\nwbsquare\n\
wcircle\nwcircumflex\nwdieresis\nwdotaccent\nwdotbelow\nwehiragana\nweierstrass\n\
wekatakana\nwekorean\nweokorean\nwgrave\nwhitebullet\nwhitecircle\nwhitecircleinverse\n\
whitecornerbracketleft\nwhitecornerbracketleftvertical\nwhitecornerbracketright\n\
whitecornerbracketrightvertical\nwhitediamond\nwhitediamondcontainingblacksmalldiamond\n\
whitedownpointingsmalltriangle\nwhitedownpointingtriangle\n\
whiteleftpointingsmalltriangle\nwhiteleftpointingtriangle\nwhitelenticularbracketleft\n\
whitelenticularbracketright\nwhiterightpointingsmalltriangle\nwhiterightpointingtriangle\n\
whitesmallsquare\nwhitesmilingface\nwhitesquare\nwhitestar\nwhitetelephone\n\
whitetortoiseshellbracketleft\nwhitetortoiseshellbracketright\n\
whiteuppointingsmalltriangle\nwhiteuppointingtriangle\nwihiragana\nwikatakana\nwikorean\n\
wmonospace\nwohiragana\nwokatakana\nwokatakanahalfwidth\nwon\nwonmonospace\nwowaenthai\n\
wparen\nwreathproduct\nwring\nwsuperior\nwturned\nwynn\nx\nxabovecmb\nxbopomofo\nxcircle\n\
xdieresis\nxdotaccent\nxeharmenian\nxi\nxmonospace\nxparen\nxsuperior\ny\nyaadosquare\n\
yabengali\nyacute\nyadeva\nyaekorean\nyagujarati\nyagurmukhi\nyahiragana\nyakatakana\n\
yakatakanahalfwidth\nyakorean\nyamakkanthai\nyasmallhiragana\nyasmallkatakana\n\
yasmallkatakanahalfwidth\nyatcyrillic\nycircle\nycircumflex\nydieresis\nydotaccent\n\
ydotbelow\nyeharabic\nyehbarreearabic\nyehbarreefinalarabic\nyehfinalarabic\n\
yehhamzaabovearabic\nyehhamzaabovefinalarabic\nyehhamzaaboveinitialarabic\n\
yehhamzaabovemedialarabic\nyehinitialarabic\nyehmedialarabic\nyehmeeminitialarabic\n\
yehmeemisolatedarabic\nyehnoonfinalarabic\nyehthreedotsbelowarabic\nyekorean\nyen\n\
yenmonospace\nyeokorean\nyeorinhieuhkorean\nyerahbenyomohebrew\nyerahbenyomolefthebrew\n\
yericyrillic\nyerudieresiscyrillic\nyesieungkorean\nyesieungpansioskorean\n\
yesieungsioskorean\nyetivhebrew\nygrave\nyhook\nyhookabove\nyiarmenian\nyicyrillic\n\
yikorean\nyinyang\nyiwnarmenian\nymonospace\nyod\nyoddagesh\nyoddageshhebrew\nyodhebrew\n\
yodyodhebrew\nyodyodpatahhebrew\nyohiragana\nyoikorean\nyokatakana\nyokatakanahalfwidth\n\
yokorean\nyosmallhiragana\nyosmallkatakana\nyosmallkatakanahalfwidth\nyotgreek\n\
yoyaekorean\nyoyakorean\nyoyakthai\nyoyingthai\nyparen\nypogegrammeni\n\
ypogegrammenigreekcmb\nyr\nyring\nysuperior\nytilde\nyturned\nyuhiragana\nyuikorean\n\
yukatakana\nyukatakanahalfwidth\nyukorean\nyusbigcyrillic\nyusbigiotifiedcyrillic\n\
yuslittlecyrillic\nyuslittleiotifiedcyrillic\nyusmallhiragana\nyusmallkatakana\n\
yusmallkatakanahalfwidth\nyuyekorean\nyuyeokorean\nyyabengali\nyyadeva\nz\nzaarmenian\n\
zacute\nzadeva\nzagurmukhi\nzaharabic\nzahfinalarabic\nzahinitialarabic\nzahiragana\n\
zahmedialarabic\nzainarabic\nzainfinalarabic\nzakatakana\nzaqefgadolhebrew\n\
zaqefqatanhebrew\nzarqahebrew\nzayin\nzayindagesh\nzayindageshhebrew\nzayinhebrew\n\
zbopomofo\nzcaron\nzcircle\nzcircumflex\nzcurl\nzdot\nzdotaccent\nzdotbelow\nzecyrillic\n\
zedescendercyrillic\nzedieresiscyrillic\nzehiragana\nzekatakana\nzero\nzeroarabic\n\
zerobengali\nzerodeva\nzerogujarati\nzerogurmukhi\nzerohackarabic\nzeroinferior\n\
zeromonospace\nzerooldstyle\nzeropersian\nzerosuperior\nzerothai\nzerowidthjoiner\n\
zerowidthnonjoiner\nzerowidthspace\nzeta\nzhbopomofo\nzhearmenian\nzhebrevecyrillic\n\
zhecyrillic\nzhedescendercyrillic\nzhedieresiscyrillic\nzihiragana\nzikatakana\n\
zinorhebrew\nzlinebelow\nzmonospace\nzohiragana\nzokatakana\nzparen\nzretroflexhook\n\
zstroke\nzuhiragana\nzukatakana\n";

/// Where each name starts in [`NAMES`].
static OFFSETS: [u32; 4495] = [
    0, 2, 5, 13, 22, 30, 37, 49, 56, 68, 83, 98, 110, 126, 138, 145, 153, 165, 182, 202, 219, 240,
    257, 274, 280, 291, 301, 311, 321, 339, 355, 370, 380, 391, 398, 410, 421, 433, 448, 454, 465,
    473, 484, 492, 498, 509, 520, 531, 538, 545, 557, 569, 571, 579, 590, 600, 611, 623, 628, 634,
    645, 656, 667, 674, 682, 684, 695, 702, 708, 719, 726, 735, 749, 763, 771, 783, 788, 799, 812,
    824, 845, 857, 887, 908, 928, 941, 963, 989, 993, 999, 1015, 1026, 1037, 1044, 1046, 1049,
    1057, 1068, 1077, 1082, 1089, 1098, 1106, 1123, 1130, 1141, 1151, 1162, 1172, 1178, 1189, 1195,
    1204, 1218, 1232, 1246, 1259, 1271, 1282, 1293, 1308, 1315, 1322, 1330, 1333, 1341, 1362, 1374,
    1387, 1389, 1396, 1408, 1415, 1422, 1436, 1448, 1456, 1468, 1485, 1502, 1522, 1539, 1560, 1577,
    1594, 1604, 1614, 1624, 1639, 1644, 1655, 1665, 1676, 1683, 1695, 1706, 1717, 1728, 1743, 1761,
    1772, 1784, 1792, 1805, 1818, 1829, 1840, 1851, 1871, 1875, 1889, 1904, 1912, 1918, 1926, 1939,
    1950, 1960, 1978, 1989, 2009, 2013, 2020, 2024, 2035, 2044, 2048, 2057, 2064, 2076, 2081, 2085,
    2094, 2106, 2108, 2117, 2126, 2134, 2142, 2150, 2158, 2169, 2181, 2191, 2197, 2210, 2220, 2231,
    2241, 2248, 2250, 2259, 2266, 2272, 2285, 2298, 2305, 2312, 2321, 2329, 2341, 2354, 2359, 2370,
    2381, 2392, 2404, 2420, 2433, 2455, 2473, 2491, 2497, 2509, 2521, 2529, 2540, 2546, 2557, 2564,
    2575, 2583, 2585, 2592, 2599, 2606, 2613, 2622, 2642, 2662, 2679, 2684, 2696, 2705, 2713, 2725,
    2735, 2746, 2756, 2767, 2778, 2789, 2796, 2809, 2827, 2836, 2838, 2849, 2852, 2863, 2870, 2882,
    2889, 2896, 2904, 2916, 2933, 2943, 2953, 2963, 2978, 2996, 3011, 3016, 3027, 3037, 3053, 3064,
    3073, 3082, 3089, 3101, 3112, 3123, 3138, 3154, 3162, 3178, 3189, 3201, 3212, 3220, 3225, 3237,
    3250, 3260, 3267, 3275, 3282, 3294, 3310, 3334, 3336, 3347, 3355, 3367, 3378, 3391, 3402, 3409,
    3411, 3420, 3429, 3447, 3454, 3465, 3485, 3500, 3506, 3523, 3548, 3555, 3564, 3572, 3585, 3595,
    3607, 3619, 3631, 3642, 3648, 3660, 3671, 3682, 3696, 3707, 3719, 3726, 3728, 3731, 3734, 3741,
    3748, 3755, 3764, 3772, 3789, 3802, 3807, 3818, 3828, 3844, 3857, 3860, 3872, 3883, 3894, 3901,
    3913, 3920, 3922, 3931, 3938, 3950, 3957, 3965, 3976, 3986, 3998, 4009, 4016, 4024, 4027, 4029,
    4032, 4039, 4046, 4055, 4063, 4080, 4093, 4104, 4114, 4117, 4127, 4137, 4140, 4152, 4163, 4174,
    4186, 4193, 4200, 4212, 4215, 4217, 4220, 4228, 4235, 4247, 4263, 4287, 4294, 4301, 4316, 4324,
    4336, 4353, 4373, 4390, 4411, 4428, 4445, 4455, 4465, 4475, 4485, 4503, 4518, 4528, 4540, 4547,
    4559, 4570, 4574, 4585, 4591, 4602, 4616, 4627, 4642, 4653, 4667, 4670, 4685, 4693, 4706, 4719,
    4725, 4739, 4750, 4769, 4788, 4799, 4807, 4820, 4831, 4840, 4848, 4862, 4868, 4875, 4887, 4899,
    4906, 4919, 4930, 4937, 4949, 4964, 4976, 4978, 4985, 4993, 5004, 5015, 5027, 5048, 5052, 5058,
    5061, 5074, 5085, 5089, 5101, 5108, 5110, 5118, 5129, 5136, 5138, 5149, 5156, 5163, 5172, 5180,
    5193, 5203, 5214, 5224, 5240, 5252, 5261, 5270, 5274, 5284, 5299, 5310, 5321, 5328, 5343, 5366,
    5368, 5377, 5386, 5395, 5404, 5413, 5422, 5431, 5440, 5449, 5458, 5467, 5476, 5485, 5494, 5503,
    5512, 5521, 5530, 5539, 5548, 5557, 5566, 5575, 5584, 5593, 5602, 5611, 5620, 5629, 5638, 5647,
    5656, 5665, 5674, 5683, 5692, 5701, 5710, 5719, 5728, 5731, 5739, 5746, 5762, 5773, 5780, 5796,
    5808, 5817, 5823, 5837, 5859, 5867, 5879, 5892, 5903, 5913, 5932, 5944, 5955, 5967, 5979, 5993,
    6004, 6017, 6029, 6035, 6044, 6055, 6072, 6079, 6091, 6093, 6097, 6102, 6109, 6118, 6126, 6143,
    6156, 6167, 6177, 6188, 6208, 6217, 6231, 6237, 6243, 6249, 6260, 6271, 6282, 6295, 6306, 6317,
    6328, 6337, 6345, 6353, 6368, 6380, 6393, 6400, 6412, 6421, 6423, 6430, 6442, 6449, 6456, 6464,
    6476, 6493, 6510, 6520, 6530, 6540, 6550, 6565, 6580, 6595, 6613, 6628, 6644, 6659, 6669, 6676,
    6688, 6699, 6705, 6716, 6730, 6741, 6756, 6767, 6781, 6803, 6818, 6829, 6837, 6853, 6869, 6880,
    6888, 6896, 6905, 6933, 6948, 6964, 6995, 7013, 7026, 7032, 7047, 7054, 7072, 7096, 7103, 7115,
    7127, 7129, 7137, 7147, 7158, 7170, 7176, 7187, 7198, 7205, 7212, 7214, 7221, 7229, 7241, 7251,
    7262, 7272, 7279, 7290, 7297, 7299, 7307, 7317, 7328, 7340, 7343, 7354, 7361, 7363, 7370, 7382,
    7394, 7402, 7414, 7424, 7439, 7450, 7460, 7473, 7494, 7501, 7507, 7518, 7529, 7540, 7553, 7564,
    7571, 7578, 7593, 7616, 7634, 7660, 7662, 7673, 7680, 7687, 7699, 7707, 7719, 7724, 7735, 7745,
    7756, 7776, 7795, 7800, 7812, 7829, 7841, 7862, 7882, 7893, 7904, 7911, 7919, 7921, 7931, 7938,
    7945, 7956, 7967, 7983, 7994, 8013, 8029, 8049, 8074, 8095, 8104, 8114, 8121, 8133, 8148, 8163,
    8175, 8191, 8203, 8210, 8218, 8230, 8247, 8267, 8284, 8305, 8322, 8328, 8342, 8351, 8361, 8371,
    8383, 8396, 8406, 8416, 8430, 8436, 8446, 8464, 8480, 8490, 8501, 8504, 8512, 8521, 8530, 8540,
    8550, 8560, 8570, 8580, 8590, 8600, 8610, 8620, 8630, 8640, 8650, 8660, 8670, 8680, 8690, 8700,
    8710, 8720, 8730, 8740, 8750, 8760, 8770, 8780, 8790, 8800, 8810, 8820, 8830, 8840, 8850, 8860,
    8870, 8880, 8890, 8900, 8910, 8920, 8930, 8940, 8950, 8960, 8970, 8980, 8990, 9000, 9010, 9020,
    9030, 9040, 9050, 9060, 9070, 9080, 9090, 9100, 9110, 9120, 9130, 9140, 9150, 9160, 9170, 9180,
    9190, 9200, 9210, 9220, 9230, 9240, 9250, 9260, 9270, 9280, 9290, 9300, 9310, 9320, 9330, 9340,
    9350, 9360, 9370, 9380, 9390, 9400, 9410, 9420, 9430, 9440, 9450, 9460, 9470, 9480, 9490, 9500,
    9510, 9520, 9530, 9540, 9550, 9560, 9570, 9580, 9590, 9600, 9610, 9618, 9626, 9634, 9644, 9654,
    9664, 9674, 9684, 9694, 9704, 9714, 9724, 9734, 9744, 9754, 9764, 9774, 9784, 9794, 9804, 9814,
    9824, 9834, 9844, 9854, 9864, 9874, 9884, 9894, 9904, 9914, 9924, 9934, 9944, 9954, 9964, 9974,
    9984, 9994, 10004, 10014, 10024, 10034, 10044, 10054, 10064, 10074, 10084, 10094, 10104, 10114,
    10124, 10134, 10144, 10154, 10164, 10174, 10184, 10194, 10204, 10214, 10224, 10234, 10244,
    10254, 10264, 10274, 10284, 10294, 10304, 10314, 10324, 10334, 10344, 10354, 10364, 10374,
    10384, 10394, 10404, 10414, 10424, 10434, 10444, 10454, 10464, 10474, 10484, 10494, 10504,
    10514, 10524, 10534, 10544, 10554, 10564, 10574, 10584, 10594, 10604, 10614, 10624, 10634,
    10644, 10654, 10664, 10674, 10684, 10694, 10704, 10714, 10724, 10734, 10744, 10754, 10764,
    10774, 10784, 10794, 10804, 10814, 10824, 10834, 10844, 10854, 10864, 10874, 10884, 10894,
    10904, 10914, 10924, 10934, 10944, 10954, 10964, 10974, 10984, 10991, 11001, 11011, 11021,
    11032, 11042, 11053, 11060, 11072, 11083, 11094, 11110, 11120, 11135, 11152, 11168, 11183,
    11202, 11218, 11238, 11248, 11267, 11275, 11280, 11291, 11308, 11324, 11345, 11371, 11392,
    11418, 11429, 11445, 11466, 11492, 11510, 11533, 11558, 11582, 11598, 11615, 11621, 11630,
    11636, 11647, 11659, 11667, 11678, 11688, 11707, 11722, 11731, 11742, 11754, 11769, 11785,
    11800, 11806, 11823, 11848, 11866, 11892, 11902, 11915, 11929, 11942, 11956, 11967, 11981,
    11996, 12010, 12025, 12034, 12044, 12057, 12073, 12086, 12103, 12111, 12124, 12131, 12150,
    12164, 12170, 12181, 12193, 12212, 12231, 12244, 12256, 12260, 12275, 12281, 12292, 12303,
    12313, 12324, 12332, 12346, 12360, 12375, 12387, 12400, 12414, 12427, 12440, 12454, 12465,
    12476, 12491, 12501, 12515, 12530, 12545, 12562, 12579, 12597, 12612, 12625, 12635, 12652,
    12665, 12684, 12703, 12720, 12735, 12750, 12765, 12776, 12794, 12814, 12830, 12849, 12867,
    12883, 12898, 12913, 12926, 12940, 12948, 12956, 12966, 12979, 12995, 13007, 13025, 13038,
    13051, 13063, 13076, 13097, 13109, 13130, 13141, 13161, 13169, 13183, 13198, 13213, 13237,
    13246, 13267, 13282, 13299, 13312, 13330, 13344, 13353, 13363, 13383, 13386, 13393, 13405,
    13413, 13421, 13431, 13442, 13449, 13460, 13471, 13491, 13507, 13526, 13542, 13562, 13575,
    13587, 13592, 13609, 13620, 13622, 13632, 13642, 13655, 13669, 13682, 13696, 13715, 13722,
    13733, 13744, 13755, 13764, 13775, 13779, 13786, 13795, 13801, 13814, 13824, 13832, 13843,
    13853, 13874, 13882, 13893, 13903, 13918, 13935, 13946, 13962, 13983, 14005, 14024, 14035,
    14047, 14051, 14056, 14072, 14082, 14098, 14108, 14122, 14133, 14141, 14153, 14165, 14171,
    14182, 14193, 14207, 14221, 14232, 14244, 14257, 14283, 14308, 14334, 14361, 14396, 14424,
    14460, 14483, 14507, 14522, 14548, 14575, 14592, 14609, 14621, 14631, 14654, 14678, 14707,
    14731, 14737, 14748, 14754, 14765, 14778, 14789, 14800, 14807, 14816, 14824, 14833, 14843,
    14856, 14870, 14883, 14897, 14909, 14922, 14936, 14955, 14970, 14982, 15000, 15011, 15025,
    15040, 15054, 15069, 15082, 15096, 15111, 15131, 15147, 15160, 15179, 15191, 15206, 15222,
    15237, 15253, 15267, 15281, 15302, 15316, 15329, 15345, 15362, 15378, 15395, 15410, 15425,
    15447, 15462, 15468, 15482, 15491, 15513, 15530, 15553, 15568, 15591, 15601, 15609, 15619,
    15627, 15638, 15649, 15656, 15670, 15685, 15694, 15696, 15707, 15717, 15724, 15731, 15742,
    15753, 15763, 15782, 15797, 15813, 15833, 15853, 15862, 15869, 15875, 15889, 15898, 15913,
    15923, 15930, 15939, 15953, 15961, 15973, 15979, 15984, 15995, 16004, 16012, 16023, 16035,
    16050, 16066, 16081, 16097, 16110, 16126, 16143, 16159, 16176, 16181, 16192, 16205, 16219,
    16232, 16245, 16257, 16268, 16276, 16288, 16300, 16311, 16332, 16342, 16354, 16384, 16405,
    16425, 16438, 16460, 16486, 16490, 16511, 16531, 16551, 16565, 16584, 16597, 16609, 16622,
    16634, 16640, 16659, 16677, 16695, 16707, 16724, 16742, 16749, 16762, 16775, 16785, 16802,
    16816, 16828, 16843, 16865, 16884, 16893, 16904, 16922, 16937, 16954, 16978, 17003, 17014,
    17033, 17047, 17053, 17067, 17079, 17092, 17107, 17112, 17126, 17140, 17154, 17165, 17181,
    17192, 17198, 17212, 17227, 17237, 17248, 17271, 17290, 17296, 17310, 17329, 17341, 17353,
    17367, 17381, 17396, 17418, 17435, 17446, 17460, 17480, 17495, 17503, 17516, 17526, 17546,
    17563, 17579, 17587, 17598, 17609, 17619, 17630, 17640, 17651, 17662, 17673, 17684, 17695,
    17706, 17716, 17727, 17738, 17749, 17760, 17771, 17781, 17791, 17801, 17811, 17821, 17832,
    17842, 17852, 17862, 17873, 17884, 17895, 17906, 17916, 17926, 17936, 17953, 17967, 17977,
    17991, 18006, 18024, 18051, 18077, 18096, 18124, 18151, 18169, 18178, 18192, 18199, 18208,
    18219, 18228, 18236, 18245, 18249, 18258, 18266, 18275, 18283, 18285, 18296, 18306, 18316,
    18323, 18338, 18355, 18371, 18378, 18391, 18398, 18408, 18419, 18430, 18441, 18452, 18462,
    18468, 18480, 18498, 18514, 18536, 18552, 18574, 18586, 18597, 18614, 18625, 18642, 18653,
    18670, 18682, 18700, 18712, 18730, 18741, 18758, 18769, 18786, 18797, 18814, 18829, 18841,
    18856, 18877, 18892, 18898, 18910, 18926, 18951, 18956, 18965, 18985, 19013, 19034, 19063,
    19087, 19100, 19114, 19129, 19145, 19154, 19163, 19175, 19187, 19198, 19212, 19227, 19239,
    19254, 19278, 19288, 19297, 19304, 19313, 19321, 19338, 19345, 19356, 19364, 19376, 19388,
    19399, 19415, 19425, 19437, 19446, 19459, 19472, 19483, 19493, 19516, 19540, 19551, 19558,
    19569, 19580, 19590, 19601, 19612, 19624, 19630, 19642, 19678, 19683, 19694, 19702, 19714,
    19726, 19732, 19747, 19765, 19773, 19782, 19794, 19811, 19820, 19834, 19851, 19863, 19877,
    19891, 19902, 19913, 19923, 19930, 19938, 19952, 19964, 19972, 19983, 19992, 20000, 20011,
    20019, 20031, 20041, 20052, 20063, 20070, 20085, 20101, 20116, 20128, 20143, 20148, 20159,
    20169, 20182, 20194, 20207, 20219, 20228, 20237, 20247, 20266, 20274, 20287, 20302, 20323,
    20340, 20352, 20359, 20369, 20375, 20383, 20394, 20405, 20408, 20417, 20425, 20432, 20453,
    20465, 20478, 20480, 20487, 20493, 20502, 20512, 20519, 20531, 20547, 20568, 20593, 20600,
    20614, 20626, 20642, 20650, 20662, 20679, 20696, 20716, 20733, 20754, 20771, 20781, 20791,
    20797, 20807, 20812, 20823, 20833, 20844, 20860, 20871, 20878, 20888, 20899, 20910, 20920,
    20931, 20942, 20948, 20960, 20973, 20985, 21013, 21023, 21038, 21052, 21067, 21081, 21095,
    21111, 21125, 21142, 21164, 21178, 21193, 21207, 21218, 21230, 21243, 21254, 21268, 21278,
    21293, 21311, 21321, 21340, 21356, 21364, 21375, 21383, 21396, 21408, 21421, 21433, 21442,
    21459, 21467, 21480, 21493, 21504, 21511, 21526, 21537, 21558, 21567, 21577, 21588, 21599,
    21606, 21621, 21641, 21645, 21657, 21671, 21686, 21694, 21702, 21711, 21717, 21729, 21743,
    21763, 21781, 21788, 21796, 21805, 21818, 21824, 21839, 21850, 21864, 21876, 21892, 21903,
    21914, 21924, 21942, 21953, 21973, 21977, 21985, 21996, 22016, 22032, 22049, 22064, 22079,
    22103, 22113, 22123, 22127, 22138, 22147, 22151, 22158, 22170, 22189, 22212, 22226, 22244,
    22252, 22261, 22266, 22284, 22299, 22318, 22325, 22340, 22350, 22361, 22377, 22393, 22405,
    22417, 22421, 22430, 22438, 22450, 22458, 22460, 22464, 22470, 22476, 22480, 22484, 22491,
    22502, 22513, 22525, 22540, 22555, 22565, 22573, 22584, 22594, 22606, 22621, 22638, 22654,
    22664, 22671, 22674, 22678, 22682, 22685, 22699, 22712, 22726, 22737, 22747, 22758, 22767,
    22782, 22803, 22818, 22833, 22854, 22868, 22888, 22897, 22912, 22921, 22936, 22944, 22958,
    22969, 22986, 23003, 23011, 23024, 23029, 23040, 23052, 23063, 23090, 23099, 23111, 23124,
    23137, 23152, 23165, 23186, 23199, 23213, 23226, 23236, 23247, 23259, 23269, 23282, 23291,
    23294, 23299, 23309, 23322, 23336, 23349, 23363, 23374, 23388, 23403, 23417, 23432, 23439,
    23450, 23459, 23469, 23478, 23486, 23499, 23511, 23518, 23523, 23534, 23546, 23557, 23584,
    23593, 23606, 23619, 23634, 23647, 23668, 23681, 23695, 23716, 23729, 23739, 23750, 23762,
    23772, 23785, 23800, 23814, 23829, 23838, 23856, 23863, 23872, 23878, 23880, 23890, 23897,
    23904, 23914, 23929, 23946, 23962, 23973, 23984, 23995, 24006, 24012, 24028, 24042, 24055,
    24065, 24072, 24079, 24088, 24096, 24108, 24121, 24126, 24137, 24148, 24159, 24170, 24189,
    24208, 24221, 24240, 24251, 24273, 24289, 24298, 24309, 24322, 24330, 24342, 24354, 24366,
    24383, 24402, 24420, 24442, 24460, 24478, 24487, 24500, 24506, 24516, 24527, 24538, 24550,
    24556, 24568, 24586, 24598, 24610, 24632, 24644, 24664, 24679, 24699, 24722, 24750, 24768,
    24794, 24802, 24813, 24824, 24835, 24842, 24852, 24861, 24867, 24881, 24890, 24900, 24910,
    24922, 24937, 24950, 24958, 24971, 24990, 25007, 25019, 25039, 25053, 25070, 25083, 25091,
    25099, 25110, 25124, 25139, 25153, 25168, 25179, 25192, 25201, 25203, 25223, 25238, 25248,
    25268, 25275, 25286, 25297, 25307, 25322, 25339, 25350, 25366, 25378, 25389, 25409, 25424,
    25436, 25453, 25473, 25490, 25510, 25525, 25545, 25568, 25585, 25598, 25615, 25633, 25649,
    25663, 25682, 25699, 25714, 25723, 25734, 25747, 25760, 25773, 25790, 25813, 25837, 25858,
    25870, 25884, 25898, 25912, 25930, 25954, 25979, 26001, 26012, 26025, 26038, 26051, 26068,
    26091, 26115, 26136, 26144, 26153, 26163, 26168, 26178, 26190, 26199, 26207, 26219, 26229,
    26240, 26250, 26253, 26259, 26266, 26281, 26296, 26305, 26320, 26336, 26346, 26355, 26376,
    26397, 26412, 26437, 26465, 26488, 26505, 26516, 26538, 26554, 26570, 26581, 26601, 26617,
    26626, 26639, 26643, 26653, 26659, 26673, 26692, 26710, 26728, 26740, 26757, 26768, 26779,
    26799, 26805, 26813, 26821, 26829, 26841, 26859, 26878, 26894, 26905, 26916, 26927, 26937,
    26948, 26959, 26979, 26985, 26993, 27001, 27009, 27021, 27039, 27058, 27074, 27087, 27101,
    27109, 27122, 27146, 27168, 27182, 27193, 27204, 27218, 27226, 27237, 27243, 27250, 27260,
    27268, 27279, 27292, 27303, 27323, 27336, 27352, 27355, 27362, 27373, 27384, 27399, 27415,
    27427, 27442, 27452, 27454, 27461, 27472, 27481, 27491, 27498, 27505, 27513, 27525, 27535,
    27545, 27566, 27586, 27611, 27632, 27656, 27673, 27690, 27711, 27742, 27767, 27789, 27816,
    27843, 27868, 27895, 27921, 27942, 27963, 27985, 28010, 28033, 28055, 28077, 28098, 28124,
    28146, 28167, 28188, 28206, 28229, 28251, 28277, 28302, 28325, 28349, 28370, 28394, 28411,
    28435, 28457, 28479, 28499, 28525, 28547, 28568, 28584, 28605, 28625, 28645, 28664, 28685,
    28705, 28711, 28721, 28736, 28754, 28764, 28780, 28791, 28810, 28828, 28846, 28858, 28875,
    28882, 28892, 28902, 28912, 28923, 28933, 28944, 28951, 28962, 28973, 28989, 29004, 29020,
    29039, 29055, 29075, 29078, 29088, 29107, 29115, 29120, 29131, 29139, 29155, 29181, 29196,
    29207, 29217, 29226, 29238, 29247, 29262, 29273, 29289, 29300, 29313, 29325, 29336, 29348,
    29364, 29377, 29397, 29412, 29429, 29440, 29450, 29460, 29473, 29484, 29492, 29497, 29510,
    29528, 29538, 29548, 29555, 29567, 29582, 29597, 29621, 29635, 29643, 29653, 29671, 29689,
    29696, 29708, 29719, 29730, 29748, 29763, 29782, 29798, 29822, 29824, 29835, 29845, 29852,
    29863, 29874, 29884, 29891, 29899, 29911, 29924, 29939, 29950, 29961, 29977, 29995, 30012,
    30022, 30037, 30048, 30056, 30068, 30080, 30093, 30097, 30108, 30115, 30125, 30127, 30145,
    30155, 30162, 30173, 30193, 30200, 30204, 30214, 30224, 30240, 30255, 30265, 30282, 30298,
    30312, 30323, 30334, 30345, 30360, 30371, 30391, 30397, 30414, 30434, 30456, 30476, 30501,
    30515, 30533, 30564, 30580, 30589, 30601, 30616, 30633, 30662, 30687, 30697, 30708, 30715,
    30724, 30732, 30745, 30755, 30767, 30778, 30789, 30809, 30821, 30837, 30850, 30861, 30873,
    30881, 30893, 30905, 30916, 30932, 30950, 30967, 30978, 30987, 31000, 31021, 31041, 31061,
    31075, 31094, 31106, 31118, 31131, 31144, 31155, 31161, 31176, 31186, 31197, 31208, 31228,
    31245, 31263, 31274, 31294, 31313, 31332, 31345, 31363, 31380, 31392, 31403, 31412, 31426,
    31437, 31453, 31464, 31475, 31485, 31496, 31516, 31528, 31542, 31563, 31574, 31581, 31591,
    31603, 31612, 31620, 31631, 31642, 31662, 31671, 31680, 31682, 31692, 31699, 31706, 31717,
    31728, 31744, 31763, 31792, 31824, 31853, 31885, 31907, 31936, 31968, 31978, 31985, 31998,
    32004, 32016, 32034, 32046, 32057, 32074, 32097, 32114, 32129, 32149, 32166, 32187, 32208,
    32232, 32248, 32272, 32293, 32318, 32343, 32355, 32366, 32371, 32377, 32387, 32394, 32403,
    32411, 32428, 32441, 32446, 32457, 32467, 32483, 32501, 32518, 32523, 32533, 32552, 32566,
    32575, 32592, 32606, 32620, 32630, 32635, 32643, 32658, 32663, 32676, 32679, 32691, 32694,
    32702, 32714, 32725, 32734, 32751, 32765, 32791, 32814, 32827, 32838, 32847, 32859, 32870,
    32888, 32903, 32914, 32933, 32943, 32960, 32974, 32985, 32991, 33009, 33020, 33034, 33042,
    33049, 33057, 33064, 33072, 33082, 33090, 33097, 33113, 33126, 33151, 33173, 33182, 33184,
    33194, 33201, 33216, 33226, 33239, 33255, 33262, 33269, 33280, 33291, 33306, 33325, 33336,
    33359, 33383, 33399, 33424, 33441, 33459, 33469, 33488, 33507, 33522, 33540, 33554, 33572,
    33591, 33602, 33622, 33640, 33659, 33670, 33690, 33703, 33714, 33734, 33739, 33753, 33765,
    33770, 33789, 33798, 33808, 33817, 33825, 33838, 33849, 33859, 33870, 33886, 33904, 33921,
    33943, 33966, 33980, 33991, 34006, 34017, 34037, 34041, 34051, 34067, 34077, 34089, 34102,
    34121, 34144, 34161, 34167, 34177, 34204, 34211, 34230, 34248, 34266, 34278, 34297, 34314,
    34331, 34347, 34358, 34369, 34389, 34395, 34409, 34421, 34430, 34440, 34447, 34463, 34474,
    34489, 34498, 34512, 34523, 34539, 34550, 34561, 34572, 34592, 34602, 34611, 34624, 34644,
    34651, 34661, 34670, 34680, 34688, 34691, 34695, 34705, 34717, 34726, 34736, 34744, 34754,
    34765, 34776, 34796, 34806, 34815, 34825, 34837, 34853, 34865, 34880, 34894, 34909, 34919,
    34929, 34939, 34952, 34961, 34974, 34983, 34985, 34995, 35001, 35008, 35015, 35026, 35037,
    35048, 35059, 35079, 35091, 35100, 35108, 35118, 35126, 35133, 35142, 35150, 35167, 35180,
    35191, 35201, 35215, 35226, 35237, 35257, 35271, 35280, 35283, 35294, 35302, 35314, 35326,
    35337, 35347, 35357, 35372, 35391, 35409, 35426, 35444, 35461, 35473, 35492, 35509, 35525,
    35543, 35554, 35565, 35585, 35602, 35615, 35620, 35631, 35643, 35654, 35681, 35690, 35703,
    35716, 35731, 35744, 35765, 35778, 35792, 35805, 35815, 35826, 35838, 35848, 35861, 35876,
    35890, 35905, 35914, 35917, 35929, 35939, 35958, 35972, 35983, 35994, 36003, 36014, 36022,
    36034, 36046, 36055, 36066, 36077, 36097, 36114, 36124, 36133, 36144, 36160, 36177, 36199,
    36220, 36238, 36260, 36283, 36300, 36322, 36345, 36365, 36377, 36388, 36401, 36410, 36421,
    36440, 36458, 36471, 36479, 36495, 36507, 36519, 36529, 36541, 36553, 36565, 36572, 36581,
    36591, 36598, 36601, 36612, 36623, 36643, 36656, 36666, 36680, 36694, 36705, 36725, 36741,
    36758, 36780, 36787, 36791, 36801, 36817, 36827, 36836, 36845, 36856, 36864, 36876, 36888,
    36890, 36897, 36906, 36914, 36930, 36954, 36963, 36973, 36980, 36992, 37008, 37029, 37054,
    37061, 37069, 37081, 37098, 37118, 37135, 37156, 37173, 37183, 37193, 37203, 37209, 37219,
    37237, 37247, 37250, 37259, 37266, 37276, 37283, 37293, 37304, 37314, 37325, 37331, 37342,
    37356, 37367, 37382, 37393, 37407, 37410, 37425, 37435, 37454, 37462, 37472, 37480, 37493,
    37506, 37513, 37519, 37526, 37540, 37557, 37576, 37595, 37606, 37617, 37625, 37638, 37649,
    37653, 37663, 37674, 37684, 37710, 37718, 37733, 37743, 37753, 37765, 37777, 37791, 37799,
    37811, 37831, 37843, 37856, 37876, 37888, 37897, 37907, 37918, 37929, 37938, 37950, 37958,
    37967, 37975, 37989, 38000, 38016, 38022, 38029, 38040, 38047, 38059, 38072, 38083, 38094,
    38114, 38121, 38133, 38148, 38163, 38187, 38200, 38210, 38221, 38228, 38240, 38255, 38266,
    38275, 38294, 38306, 38321, 38337, 38350, 38360, 38378, 38393, 38412, 38418, 38420, 38433,
    38448, 38458, 38465, 38472, 38481, 38488, 38499, 38510, 38521, 38535, 38546, 38572, 38589,
    38603, 38613, 38622, 38632, 38645, 38659, 38681, 38694, 38708, 38720, 38732, 38745, 38763,
    38782, 38797, 38815, 38827, 38845, 38856, 38870, 38885, 38908, 38922, 38937, 38950, 38963,
    38977, 38996, 39016, 39032, 39051, 39064, 39083, 39095, 39107, 39120, 39129, 39135, 39143,
    39151, 39159, 39171, 39189, 39208, 39224, 39236, 39246, 39254, 39265, 39268, 39279, 39288,
    39303, 39315, 39335, 39345, 39357, 39366, 39381, 39398, 39409, 39425, 39436, 39457, 39470,
    39478, 39492, 39509, 39522, 39529, 39544, 39559, 39575, 39590, 39606, 39618, 39633, 39653,
    39667, 39682, 39694, 39701, 39710, 39721, 39729, 39741, 39753, 39757, 39762, 39783, 39803,
    39823, 39837, 39856, 39865, 39877, 39892, 39898, 39910, 39923, 39938, 39941, 39945, 39964,
    39982, 39999, 40017, 40035, 40047, 40064, 40086, 40102, 40124, 40143, 40161, 40172, 40183,
    40197, 40210, 40215, 40228, 40239, 40249, 40257, 40271, 40281, 40294, 40305, 40314, 40325,
    40348, 40371, 40395, 40416, 40427, 40437, 40448, 40463, 40470, 40479, 40493, 40506, 40512,
    40521, 40535, 40543, 40558, 40570, 40581, 40595, 40605, 40618, 40633, 40644, 40657, 40661,
    40673, 40698, 40707, 40718, 40729, 40739, 40748, 40757, 40759, 40766, 40778, 40788, 40803,
    40820, 40836, 40843, 40852, 40861, 40870, 40879, 40888, 40897, 40906, 40919, 40938, 40956,
    40980, 41005, 41027, 41047, 41064, 41081, 41091, 41099, 41105, 41116, 41120, 41130, 41146,
    41160, 41180, 41194, 41214, 41224, 41233, 41248, 41257, 41272, 41281, 41296, 41306, 41322,
    41332, 41348, 41357, 41372, 41381, 41396, 41405, 41420, 41427, 41439, 41446, 41455, 41464,
    41473, 41486, 41505, 41525, 41542, 41551, 41566, 41583, 41596, 41614, 41628, 41646, 41660,
    41669, 41682, 41695, 41713, 41727, 41749, 41763, 41773, 41791, 41805, 41816, 41828, 41843,
    41855, 41876, 41878, 41889, 41899, 41906, 41913, 41921, 41932, 41944, 41955, 41967, 41977,
    41987, 42002, 42024, 42034, 42039, 42050, 42061, 42072, 42083, 42094, 42114, 42137, 42161,
    42170, 42180, 42186, 42196, 42203, 42212, 42220, 42233, 42243, 42254, 42264, 42280, 42294,
    42307, 42322, 42333, 42346, 42360, 42370, 42382, 42397, 42408, 42428, 42439, 42459, 42464,
    42481, 42496, 42517, 42532, 42553, 42564, 42574, 42590, 42600, 42616, 42626, 42642, 42653,
    42670, 42681, 42698, 42708, 42724, 42734, 42750, 42760, 42776, 42790, 42802, 42821, 42835,
    42845, 42863, 42874, 42882, 42886, 42891, 42897, 42909, 42929, 42944, 42958, 42977, 42995,
    43013, 43030, 43048, 43070, 43082, 43099, 43118, 43135, 43154, 43171, 43192, 43208, 43227,
    43245, 43268, 43279, 43297, 43311, 43322, 43333, 43353, 43358, 43371, 43379, 43392, 43413,
    43434, 43455, 43469, 43491, 43513, 43528, 43542, 43553, 43562, 43577, 43588, 43599, 43610,
    43630, 43640, 43647, 43658, 43666, 43678, 43689, 43705, 43722, 43736, 43754, 43780, 43803,
    43830, 43840, 43848, 43856, 43872, 43883, 43894, 43914, 43931, 43948, 43955, 43962, 43978,
    43991, 44008, 44033, 44055, 44081, 44083, 44093, 44100, 44116, 44126, 44133, 44148, 44165,
    44181, 44192, 44203, 44214, 44225, 44245, 44277, 44284, 44297, 44316, 44329, 44340, 44351,
    44370, 44388, 44399, 44409, 44419, 44434, 44445, 44459, 44469, 44479, 44495, 44507, 44522,
    44533, 44543, 44554, 44564, 44571, 44587, 44596, 44602, 44616, 44638, 44648, 44656, 44668,
    44681, 44692, 44702, 44721, 44737, 44744, 44762, 44770, 44781, 44797, 44815, 44832, 44838,
    44846, 44854, 44862, 44874, 44892, 44911, 44925, 44941, 44953, 44964, 44975, 44995, 45005,
    45021, 45040, 45055, 45074, 45102, 45114, 45126, 45132, 45144, 45157, 45169, 45197, 45207,
    45220, 45234, 45248, 45264, 45278, 45300, 45314, 45329, 45343, 45354, 45366, 45379, 45390,
    45404, 45420, 45435, 45451, 45461, 45471, 45483, 45494, 45506, 45519, 45537, 45558, 45576,
    45597, 45615, 45636, 45642, 45652, 45663, 45675, 45683, 45695, 45707, 45724, 45730, 45741,
    45755, 45767, 45784, 45803, 45821, 45832, 45839, 45852, 45858, 45867, 45875, 45883, 45891,
    45903, 45921, 45940, 45956, 45969, 45981, 45986, 45997, 46014, 46032, 46056, 46073, 46096,
    46110, 46121, 46133, 46151, 46162, 46179, 46185, 46191, 46198, 46209, 46232, 46243, 46254,
    46274, 46286, 46302, 46310, 46323, 46336, 46354, 46371, 46387, 46404, 46421, 46432, 46448,
    46464, 46480, 46497, 46501, 46511, 46522, 46532, 46558, 46566, 46578, 46590, 46604, 46616,
    46636, 46648, 46661, 46673, 46682, 46692, 46703, 46712, 46724, 46738, 46772, 46785, 46799,
    46807, 46813, 46822, 46832, 46841, 46851, 46866, 46872, 46887, 46897, 46907, 46917, 46928,
    46943, 46954, 46971, 46982, 46993, 47013, 47035, 47058, 47069, 47080, 47089, 47099, 47105,
    47121, 47127, 47142, 47157, 47164, 47179, 47188, 47197, 47226, 47247, 47256, 47265, 47281,
    47290, 47300, 47309, 47319, 47328, 47343, 47374, 47406, 47438, 47457, 47483, 47492, 47503,
    47511, 47523, 47540, 47557, 47574, 47592, 47609, 47626, 47642, 47660, 47670, 47675, 47684,
    47702, 47723, 47745, 47752, 47767, 47781, 47795, 47804, 47813, 47824, 47835, 47855, 47867,
    47877, 47894, 47908, 47912, 47921, 47938, 47954, 47970, 47979, 47995, 47997, 48007, 48016,
    48025, 48032, 48043, 48054, 48064, 48079, 48096, 48107, 48123, 48140, 48151, 48171, 48185,
    48189, 48193, 48202, 48212, 48228, 48238, 48243, 48253, 48260, 48267, 48276, 48288, 48305,
    48324, 48342, 48365, 48373, 48390, 48403, 48413, 48424, 48434, 48445, 48465, 48475, 48490,
    48510, 48531, 48548, 48559, 48580, 48602, 48619, 48641, 48657, 48678, 48700, 48719, 48730,
    48750, 48760, 48775, 48795, 48815, 48825, 48845, 48854, 48864, 48873, 48878, 48882, 48892,
    48908, 48918, 48932, 48944, 48960, 48971, 48979, 48991, 49003, 49014, 49030, 49053, 49077,
    49093, 49118, 49129, 49145, 49163, 49180, 49192, 49202, 49208, 49215, 49232, 49253, 49273,
    49293, 49307, 49326, 49341, 49355, 49370, 49388, 49394, 49409, 49415, 49429, 49441, 49454,
    49467, 49484, 49509, 49535, 49541, 49553, 49566, 49578, 49606, 49616, 49629, 49643, 49657,
    49673, 49687, 49709, 49723, 49738, 49760, 49774, 49785, 49797, 49810, 49824, 49844, 49855,
    49869, 49879, 49889, 49893, 49904, 49915, 49935, 49955, 49974, 49993, 50006, 50024, 50030,
    50044, 50053, 50063, 50078, 50092, 50108, 50125, 50135, 50146, 50158, 50170, 50183, 50200,
    50214, 50231, 50244, 50255, 50266, 50277, 50288, 50299, 50319, 50339, 50358, 50373, 50387,
    50401, 50410, 50418, 50426, 50432, 50442, 50454, 50479, 50509, 50542, 50568, 50599, 50633,
    50643, 50656, 50663, 50673, 50687, 50702, 50717, 50725, 50733, 50741, 50749, 50758, 50770,
    50783, 50797, 50800, 50806, 50818, 50836, 50848, 50860, 50866, 50874, 50882, 50890, 50902,
    50920, 50939, 50955, 50968, 50978, 50989, 50997, 51009, 51021, 51032, 51048, 51066, 51083,
    51095, 51104, 51117, 51130, 51138, 51149, 51160, 51180, 51194, 51209, 51225, 51241, 51266,
    51279, 51291, 51304, 51316, 51328, 51341, 51356, 51368, 51381, 51385, 51395, 51406, 51416,
    51442, 51450, 51465, 51478, 51499, 51511, 51523, 51537, 51549, 51569, 51581, 51594, 51614,
    51626, 51635, 51645, 51656, 51665, 51675, 51687, 51695, 51705, 51707, 51714, 51719, 51728,
    51738, 51745, 51752, 51760, 51772, 51789, 51799, 51810, 51820, 51830, 51836, 51846, 51861,
    51876, 51891, 51909, 51924, 51940, 51950, 51957, 51967, 51977, 51987, 51998, 52004, 52015,
    52029, 52040, 52055, 52066, 52080, 52102, 52117, 52127, 52146, 52157, 52165, 52173, 52189,
    52205, 52220, 52231, 52242, 52256, 52276, 52295, 52310, 52316, 52329, 52340, 52358, 52373,
    52381, 52396, 52408, 52418, 52428, 52436, 52443, 52451, 52466, 52474, 52490, 52511, 52524,
    52537, 52552, 52562, 52574, 52580, 52595, 52610, 52625, 52649, 52667, 52691, 52698, 52710,
    52722, 52732, 52739, 52750, 52761, 52777, 52796, 52812, 52832, 52850, 52865, 52884, 52886,
    52893, 52904, 52915, 52926, 52930, 52940, 52952, 52968, 52978, 52987, 53002, 53015, 53028,
    53036, 53046, 53053, 53064, 53074, 53089, 53106, 53122, 53133, 53139, 53151, 53172, 53193,
    53212, 53228, 53240, 53246, 53257, 53271, 53282, 53297, 53312, 53324, 53340, 53353, 53365,
    53376, 53387, 53411, 53435, 53450, 53474, 53485, 53492, 53499, 53507, 53518, 53529, 53531,
    53538, 53548, 53559, 53570, 53590, 53599, 53615, 53631, 53643, 53652, 53675, 53685, 53700,
    53720, 53745, 53754, 53762, 53774, 53784, 53795, 53805, 53816, 53828, 53839, 53848, 53858,
    53865, 53877, 53889, 53908, 53931, 53962, 53986, 54018, 54031, 54071, 54102, 54128, 54159,
    54185, 54212, 54240, 54272, 54299, 54316, 54333, 54345, 54355, 54370, 54400, 54431, 54460,
    54484, 54495, 54506, 54515, 54526, 54537, 54548, 54568, 54572, 54585, 54596, 54603, 54617,
    54623, 54633, 54641, 54646, 54648, 54658, 54668, 54676, 54686, 54697, 54709, 54712, 54723,
    54730, 54740, 54742, 54754, 54764, 54771, 54778, 54788, 54799, 54810, 54821, 54832, 54852,
    54861, 54874, 54890, 54906, 54931, 54943, 54951, 54963, 54973, 54984, 54994, 55004, 55020,
    55041, 55056, 55076, 55101, 55128, 55154, 55171, 55187, 55208, 55230, 55249, 55273, 55282,
    55286, 55299, 55309, 55327, 55346, 55369, 55382, 55403, 55418, 55440, 55459, 55471, 55478,
    55484, 55495, 55506, 55517, 55526, 55534, 55547, 55558, 55562, 55572, 55588, 55598, 55611,
    55629, 55640, 55650, 55661, 55681, 55690, 55706, 55722, 55747, 55756, 55768, 55779, 55789,
    55800, 55807, 55821, 55843, 55846, 55852, 55862, 55869, 55877, 55888, 55898, 55909, 55929,
    55938, 55953, 55976, 55994, 56020, 56036, 56052, 56077, 56088, 56100, 56111, 56119, 56121,
    56132, 56139, 56146, 56157, 56167, 56182, 56199, 56210, 56226, 56237, 56253, 56264, 56281,
    56298, 56310, 56316, 56328, 56346, 56358, 56368, 56375, 56383, 56395, 56401, 56406, 56417,
    56427, 56438, 56458, 56477, 56488, 56499, 56504, 56515, 56527, 56536, 56549, 56562, 56577,
    56590, 56604, 56617, 56629, 56642, 56651, 56667, 56686, 56701, 56706, 56717, 56729, 56746,
    56758, 56779, 56799, 56810, 56821, 56833, 56844, 56855, 56866, 56877, 56884, 56899, 56907,
    56918,
];

static CODES: [u16; 4495] = [
    0x0041, 0x00C6, 0x01FC, 0x01E2, 0xF7E6, 0x00C1, 0xF7E1, 0x0102, 0x1EAE, 0x04D0, 0x1EB6, 0x1EB0,
    0x1EB2, 0x1EB4, 0x01CD, 0x24B6, 0x00C2, 0x1EA4, 0x1EAC, 0x1EA6, 0x1EA8, 0xF7E2, 0x1EAA, 0xF6C9,
    0xF7B4, 0x0410, 0x0200, 0x00C4, 0x04D2, 0x01DE, 0xF7E4, 0x1EA0, 0x01E0, 0x00C0, 0xF7E0, 0x1EA2,
    0x04D4, 0x0202, 0x0391, 0x0386, 0x0100, 0xFF21, 0x0104, 0x00C5, 0x01FA, 0x1E00, 0xF7E5, 0xF761,
    0x00C3, 0xF7E3, 0x0531, 0x0042, 0x24B7, 0x1E02, 0x1E04, 0x0411, 0x0532, 0x0392, 0x0181, 0x1E06,
    0xFF22, 0xF6F4, 0xF762, 0x0182, 0x0043, 0x053E, 0x0106, 0xF6CA, 0xF6F5, 0x010C, 0x00C7, 0x1E08,
    0xF7E7, 0x24B8, 0x0108, 0x010A, 0x010A, 0xF7B8, 0x0549, 0x04BC, 0x0427, 0x04BE, 0x04B6, 0x04F4,
    0x0543, 0x04CB, 0x04B8, 0x03A7, 0x0187, 0xF6F6, 0xFF23, 0x0551, 0xF763, 0x0044, 0x01F1, 0x01C4,
    0x0534, 0x0189, 0x0110, 0x010E, 0x1E10, 0x24B9, 0x1E12, 0x0110, 0x1E0A, 0x1E0C, 0x0414, 0x03EE,
    0x2206, 0x0394, 0x018A, 0xF6CB, 0xF6CC, 0xF6CD, 0xF7A8, 0x03DC, 0x0402, 0x1E0E, 0xFF24, 0xF6F7,
    0x0110, 0xF764, 0x018B, 0x01F2, 0x01C5, 0x04E0, 0x0405, 0x040F, 0x0045, 0x00C9, 0xF7E9, 0x0114,
    0x011A, 0x1E1C, 0x0535, 0x24BA, 0x00CA, 0x1EBE, 0x1E18, 0x1EC6, 0x1EC0, 0x1EC2, 0xF7EA, 0x1EC4,
    0x0404, 0x0204, 0x00CB, 0xF7EB, 0x0116, 0x0116, 0x1EB8, 0x0424, 0x00C8, 0xF7E8, 0x0537, 0x1EBA,
    0x2167, 0x0206, 0x0464, 0x041B, 0x216A, 0x0112, 0x1E16, 0x1E14, 0x041C, 0xFF25, 0x041D, 0x04A2,
    0x014A, 0x04A4, 0x04C7, 0x0118, 0x0190, 0x0395, 0x0388, 0x0420, 0x018E, 0x042D, 0x0421, 0x04AA,
    0x01A9, 0xF765, 0x0397, 0x0538, 0x0389, 0x00D0, 0xF7F0, 0x1EBC, 0x1E1A, 0x20AC, 0x01B7, 0x01EE,
    0x01B8, 0x0046, 0xD807, 0xD808, 0xD804, 0xD805, 0xD806, 0x24BB, 0x1E1E, 0x0556, 0x03E4, 0x0191,
    0x0472, 0x2164, 0xFF26, 0x2163, 0xF766, 0x0047, 0x3387, 0x01F4, 0x0393, 0x0194, 0x03EA, 0x011E,
    0x01E6, 0x0122, 0x24BC, 0x011C, 0x0122, 0x0120, 0x0120, 0x0413, 0x0053, 0x1E9E, 0xD803, 0x0542,
    0x0494, 0x0492, 0x0490, 0x0193, 0x0533, 0x0403, 0x1E20, 0xFF27, 0xF6CE, 0xF760, 0xF767, 0x029B,
    0x01E4, 0x0048, 0x25CF, 0x25AA, 0x25AB, 0x25A1, 0x33CB, 0x04A8, 0x04B2, 0x042A, 0x0126, 0x1E2A,
    0x1E28, 0x24BD, 0x0124, 0x1E26, 0x1E22, 0x1E24, 0xFF28, 0x0540, 0x03E8, 0xF768, 0xF6CF, 0xF6F8,
    0x3390, 0x0049, 0x042F, 0x0132, 0x042E, 0x00CD, 0xF7ED, 0x012C, 0x01CF, 0x24BE, 0x00CE, 0xF7EE,
    0x0406, 0x0208, 0x00CF, 0x1E2E, 0x04E4, 0xF7EF, 0x0130, 0x0130, 0x1ECA, 0x04D6, 0x0415, 0x2111,
    0x2111, 0x00CC, 0xF7EC, 0x1EC8, 0x0418, 0x020A, 0x0419, 0x012A, 0x04E2, 0xFF29, 0x053B, 0x0401,
    0x012E, 0x0399, 0x0196, 0x03AA, 0x038A, 0xF769, 0x0197, 0x0128, 0x1E2C, 0x0474, 0x0476, 0x004A,
    0x0541, 0x24BF, 0x0134, 0x0408, 0x054B, 0xFF2A, 0xF76A, 0x004B, 0x3385, 0x33CD, 0x04A0, 0x1E30,
    0x041A, 0x049A, 0x04C3, 0x039A, 0x049E, 0x049C, 0x01E8, 0x0136, 0x24C0, 0x0136, 0x1E32, 0x0554,
    0x053F, 0x0425, 0x03E6, 0x0198, 0x040C, 0x1E34, 0xFF2B, 0x0480, 0x03DE, 0x046E, 0xF76B, 0x004C,
    0x01C7, 0xF6BF, 0x0139, 0x039B, 0x013D, 0x013B, 0x24C1, 0x1E3C, 0x013B, 0x013F, 0x013F, 0x1E36,
    0x1E38, 0x053C, 0x01C8, 0x0409, 0x1E3A, 0xFF2C, 0x0141, 0xF6F9, 0xF76C, 0x004D, 0x3386, 0xF6D0,
    0xF7AF, 0x1E3E, 0x24C2, 0x1E40, 0x1E42, 0x0544, 0xFF2D, 0xF76D, 0x019C, 0x039C, 0x004E, 0x01CA,
    0x0143, 0x0147, 0x0145, 0x24C3, 0x1E4A, 0x0145, 0x1E44, 0x1E46, 0x014A, 0x019D, 0x2168, 0x01CB,
    0x040A, 0x1E48, 0xFF2E, 0x0546, 0xF76E, 0x00D1, 0xF7F1, 0x039D, 0x004F, 0x0152, 0xF6FA, 0x00D3,
    0xF7F3, 0x04E8, 0x04EA, 0x014E, 0x01D1, 0x019F, 0x24C4, 0x00D4, 0x1ED0, 0x1ED8, 0x1ED2, 0x1ED4,
    0xF7F4, 0x1ED6, 0x041E, 0x0150, 0x020C, 0x00D6, 0x04E6, 0xF7F6, 0x1ECC, 0xF6FB, 0x00D2, 0xF7F2,
    0x0555, 0x2126, 0x1ECE, 0x01A0, 0x1EDA, 0x1EE2, 0x1EDC, 0x1EDE, 0x1EE0, 0x0150, 0x01A2, 0x020E,
    0x014C, 0x1E52, 0x1E50, 0x2126, 0x0460, 0x03A9, 0x047A, 0x047C, 0x038F, 0x039F, 0x038C, 0xFF2F,
    0x2160, 0x01EA, 0x01EC, 0x0186, 0x00D8, 0x01FE, 0xF7F8, 0xF76F, 0x01FE, 0x047E, 0x00D5, 0x1E4C,
    0x1E4E, 0xF7F5, 0x0050, 0x1E54, 0x24C5, 0x1E56, 0x041F, 0x054A, 0x04A6, 0x03A6, 0x01A4, 0x03A0,
    0x0553, 0xFF30, 0x03A8, 0x0470, 0xF770, 0x0051, 0x24C6, 0xFF31, 0xF771, 0x0052, 0x054C, 0x0154,
    0x0158, 0x0156, 0x24C7, 0x0156, 0x0210, 0x1E58, 0x1E5A, 0x1E5C, 0x0550, 0x211C, 0x211C, 0x03A1,
    0xF6FC, 0x0212, 0x1E5E, 0xFF32, 0xF772, 0x0281, 0x02B6, 0x0053, 0x250C, 0x2514, 0x2510, 0x2518,
    0x253C, 0x252C, 0x2534, 0x251C, 0x2524, 0x2500, 0x2502, 0x2561, 0x2562, 0x2556, 0x2555, 0x2563,
    0x2551, 0x2557, 0x255D, 0x255C, 0x255B, 0x255E, 0x255F, 0x255A, 0x2554, 0x2569, 0x2566, 0x2560,
    0x2550, 0x256C, 0x2567, 0x2568, 0x2564, 0x2565, 0x2559, 0x2558, 0x2552, 0x2553, 0x256B, 0x256A,
    0x0053, 0xD803, 0x015A, 0x1E64, 0x03E0, 0x0160, 0x1E66, 0xF6FD, 0x015E, 0x018F, 0x04D8, 0x04DA,
    0x24C8, 0x015C, 0x0218, 0x1E60, 0x1E62, 0x1E68, 0x054D, 0x2166, 0x0547, 0x0428, 0x0429, 0x03E2,
    0x04BA, 0x03EC, 0x03A3, 0x2165, 0xFF33, 0x042C, 0xF773, 0x03DA, 0x0054, 0x03A4, 0x0166, 0x0164,
    0x0162, 0x24C9, 0x1E70, 0x0162, 0x1E6A, 0x1E6C, 0x0422, 0x04AC, 0x2169, 0x04B4, 0x0398, 0x01AC,
    0x00DE, 0xF7FE, 0x2162, 0xF6FE, 0x054F, 0x1E6E, 0xFF34, 0x0539, 0x01BC, 0x0184, 0x01A7, 0x01AE,
    0x0426, 0x040B, 0xF774, 0x216B, 0x2161, 0x0055, 0x00DA, 0xF7FA, 0x016C, 0x01D3, 0x24CA, 0x00DB,
    0x1E76, 0xF7FB, 0x0423, 0x0170, 0x0214, 0x00DC, 0x01D7, 0x1E72, 0x01D9, 0x04F0, 0x01DB, 0x01D5,
    0xF7FC, 0x1EE4, 0x00D9, 0xF7F9, 0x1EE6, 0x01AF, 0x1EE8, 0x1EF0, 0x1EEA, 0x1EEC, 0x1EEE, 0x0170,
    0x04F2, 0x0216, 0x0478, 0x016A, 0x04EE, 0x1E7A, 0xFF35, 0x0172, 0x03A5, 0x03D2, 0x03D3, 0x01B1,
    0x03AB, 0x03D4, 0x03D2, 0x038E, 0x016E, 0x040E, 0xF775, 0x04AE, 0x04B0, 0x0168, 0x1E78, 0x1E74,
    0x0056, 0x24CB, 0x1E7E, 0x0412, 0x054E, 0x01B2, 0xFF36, 0x0548, 0xF776, 0x1E7C, 0x0057, 0x1E82,
    0x24CC, 0x0174, 0x1E84, 0x1E86, 0x1E88, 0x1E80, 0xFF37, 0xF777, 0x0058, 0x24CD, 0x1E8C, 0x1E8A,
    0x053D, 0x039E, 0xFF38, 0xF778, 0x0059, 0x00DD, 0xF7FD, 0x0462, 0x24CE, 0x0176, 0x0178, 0xF7FF,
    0x1E8E, 0x1EF4, 0x042B, 0x04F8, 0x1EF2, 0x01B3, 0x1EF6, 0x0545, 0x0407, 0x0552, 0xFF39, 0xF779,
    0x1EF8, 0x046A, 0x046C, 0x0466, 0x0468, 0x005A, 0x0536, 0x0179, 0x017D, 0xF6FF, 0x24CF, 0x1E90,
    0x017B, 0x017B, 0x1E92, 0x0417, 0x0498, 0x04DE, 0x0396, 0x053A, 0x04C1, 0x0416, 0x0496, 0x04DC,
    0x1E94, 0xFF3A, 0xF77A, 0x01B5, 0x0061, 0x0986, 0x00E1, 0x0906, 0x0A86, 0x0A06, 0x0A3E, 0x3303,
    0x09BE, 0x093E, 0x0ABE, 0x055F, 0x0970, 0x0985, 0x311A, 0x0103, 0x1EAF, 0x04D1, 0x1EB7, 0x1EB1,
    0x1EB3, 0x1EB5, 0x01CE, 0x24D0, 0x00E2, 0x1EA5, 0x1EAD, 0x1EA7, 0x1EA9, 0x1EAB, 0x00B4, 0x0317,
    0x0301, 0x0301, 0x0954, 0x02CF, 0x0341, 0x0430, 0x0201, 0x0A71, 0x0905, 0x00E4, 0x04D3, 0x01DF,
    0x1EA1, 0x01E1, 0x00E6, 0x01FD, 0x3150, 0x01E3, 0x2015, 0x20A4, 0x0410, 0x0411, 0x0412, 0x0413,
    0x0414, 0x0415, 0x0401, 0x0416, 0x0417, 0x0418, 0x0419, 0x041A, 0x041B, 0x041C, 0x041D, 0x041E,
    0x041F, 0x0420, 0x0421, 0x0422, 0x0423, 0x0424, 0x0425, 0x0426, 0x0427, 0x0428, 0x0429, 0x042A,
    0x042B, 0x042C, 0x042D, 0x042E, 0x042F, 0x0490, 0x0402, 0x0403, 0x0404, 0x0405, 0x0406, 0x0407,
    0x0408, 0x0409, 0x040A, 0x040B, 0x040C, 0x040E, 0xF6C4, 0xF6C5, 0x0430, 0x0431, 0x0432, 0x0433,
    0x0434, 0x0435, 0x0451, 0x0436, 0x0437, 0x0438, 0x0439, 0x043A, 0x043B, 0x043C, 0x043D, 0x043E,
    0x043F, 0x0440, 0x0441, 0x0442, 0x0443, 0x0444, 0x0445, 0x0446, 0x0447, 0x0448, 0x0449, 0x044A,
    0x044B, 0x044C, 0x044D, 0x044E, 0x044F, 0x0491, 0x0452, 0x0453, 0x0454, 0x0455, 0x0456, 0x0457,
    0x0458, 0x0459, 0x045A, 0x045B, 0x045C, 0x045E, 0x040F, 0x0462, 0x0472, 0x0474, 0xF6C6, 0x045F,
    0x0463, 0x0473, 0x0475, 0xF6C7, 0xF6C8, 0x04D9, 0x200E, 0x200F, 0x200D, 0x066A, 0x060C, 0x0660,
    0x0661, 0x0662, 0x0663, 0x0664, 0x0665, 0x0666, 0x0667, 0x0668, 0x0669, 0x061B, 0x061F, 0x0621,
    0x0622, 0x0623, 0x0624, 0x0625, 0x0626, 0x0627, 0x0628, 0x0629, 0x062A, 0x062B, 0x062C, 0x062D,
    0x062E, 0x062F, 0x0630, 0x0631, 0x0632, 0x0633, 0x0634, 0x0635, 0x0636, 0x0637, 0x0638, 0x0639,
    0x063A, 0x0640, 0x0641, 0x0642, 0x0643, 0x0644, 0x0645, 0x0646, 0x0648, 0x0649, 0x064A, 0x064B,
    0x064C, 0x064D, 0x064E, 0x064F, 0x0650, 0x0651, 0x0652, 0x0647, 0x06A4, 0x067E, 0x0686, 0x0698,
    0x06AF, 0x0679, 0x0688, 0x0691, 0x06BA, 0x06D2, 0x06D5, 0x20AA, 0x05BE, 0x05C3, 0x05D0, 0x05D1,
    0x05D2, 0x05D3, 0x05D4, 0x05D5, 0x05D6, 0x05D7, 0x05D8, 0x05D9, 0x05DA, 0x05DB, 0x05DC, 0x05DD,
    0x05DE, 0x05DF, 0x05E0, 0x05E1, 0x05E2, 0x05E3, 0x05E4, 0x05E5, 0x05E6, 0x05E7, 0x05E8, 0x05E9,
    0x05EA, 0xFB2A, 0xFB2B, 0xFB4B, 0xFB1F, 0x05F0, 0x05F1, 0x05F2, 0xFB35, 0x05B4, 0x05B5, 0x05B6,
    0x05BB, 0x05B8, 0x05B7, 0x05B0, 0x05B2, 0x05B1, 0x05B3, 0x05C2, 0x05C1, 0x05B9, 0x05BC, 0x05BD,
    0x05BF, 0x05C0, 0x02BC, 0x2105, 0x2113, 0x2116, 0x202C, 0x202D, 0x202E, 0x200C, 0x066D, 0x02BD,
    0x00E0, 0x0A85, 0x0A05, 0x3042, 0x1EA3, 0x0990, 0x311E, 0x0910, 0x04D5, 0x0A90, 0x0A10, 0x0A48,
    0x0639, 0xFECA, 0xFECB, 0xFECC, 0x0203, 0x09C8, 0x0948, 0x0AC8, 0x30A2, 0xFF71, 0x314F, 0x05D0,
    0x0627, 0xFB30, 0xFE8E, 0x0623, 0xFE84, 0x0625, 0xFE88, 0x05D0, 0xFB4F, 0x0622, 0xFE82, 0x0649,
    0xFEF0, 0xFEF3, 0xFEF4, 0xFB2E, 0xFB2F, 0x2135, 0x224C, 0x03B1, 0x03AC, 0xD802, 0x0101, 0xFF41,
    0x0026, 0xFF06, 0xF726, 0x33C2, 0x3122, 0x3124, 0x27E8, 0x27E9, 0x0E5A, 0x2220, 0x3008, 0xFE3F,
    0x3009, 0xFE40, 0x2329, 0x2329, 0x2329, 0x2329, 0x2329, 0x232A, 0x232A, 0x232A, 0x232A, 0x232A,
    0x212B, 0x0387, 0x0952, 0x0982, 0x0902, 0x0A82, 0x0105, 0x3300, 0x249C, 0x055A, 0x02BC, 0xF8FF,
    0x2250, 0x2248, 0x2252, 0x2245, 0x318E, 0x318D, 0x2312, 0x1E9A, 0x00E5, 0x01FB, 0x1E01, 0x2194,
    0x2195, 0x2193, 0x21E3, 0x21E0, 0x21E2, 0x21E1, 0x21D4, 0x21D5, 0x21D3, 0x21D0, 0x21D2, 0x21D1,
    0x21D1, 0x21D5, 0x2193, 0x2199, 0x2198, 0x21E9, 0x02C5, 0x02C2, 0x02C3, 0x02C4, 0xF8E7, 0x2190,
    0x21BD, 0x21D0, 0x21CD, 0x21C6, 0x21BC, 0x21E6, 0x2197, 0x2196, 0x2192, 0x21C1, 0x21CF, 0x279E,
    0x21C4, 0x21C0, 0x21E8, 0x2198, 0x2199, 0x21E4, 0x21E5, 0x2191, 0x2191, 0x2195, 0x21A8, 0x21A8,
    0x2196, 0x21C5, 0x2197, 0x21E7, 0x2195, 0xF8E6, 0xD80A, 0x005E, 0xFF3E, 0x007E, 0xFF5E, 0x0251,
    0x0252, 0x3041, 0x30A1, 0xFF67, 0x002A, 0x066D, 0x066D, 0x2217, 0x2217, 0xFF0A, 0xFE61, 0x2042,
    0xF6E9, 0x2243, 0x0040, 0x00E3, 0xFF20, 0xFE6B, 0x0250, 0x0994, 0x3120, 0x0914, 0x0A94, 0x0A14,
    0x09D7, 0x0A4C, 0x09CC, 0x094C, 0x0ACC, 0x093D, 0x0561, 0x05E2, 0xFB20, 0x05E2, 0x0062, 0x09AC,
    0x005C, 0x005C, 0x005C, 0x005C, 0x005C, 0xFF3C, 0x092C, 0x0AAC, 0x0A2C, 0x3070, 0x0E3F, 0x30D0,
    0x007C, 0x2225, 0x2016, 0x007C, 0xFF5C, 0x3105, 0x24D1, 0x1E03, 0x1E05, 0x266C, 0x2235, 0x0431,
    0x0628, 0xFE90, 0xFE91, 0x3079, 0xFE92, 0xFC9F, 0xFC08, 0xFC6D, 0x30D9, 0x0562, 0x05D1, 0x03B2,
    0x03D0, 0xFB31, 0xFB31, 0x05D1, 0xFB4C, 0x09AD, 0x092D, 0x0AAD, 0x0A2D, 0x0253, 0x3073, 0x30D3,
    0x0298, 0x0A02, 0x3331, 0x25CF, 0x25C6, 0x25BC, 0x25C4, 0x25C0, 0x3010, 0xFE3B, 0x3011, 0xFE3C,
    0x25E3, 0x25E2, 0x25AC, 0x25BA, 0x25B6, 0x25AA, 0x263B, 0x25A0, 0x2605, 0x25E4, 0x25E5, 0x25B4,
    0x25B2, 0x2423, 0x1E07, 0x2588, 0xFF42, 0x0E1A, 0x307C, 0x30DC, 0x249D, 0x33C3, 0x007C, 0xF8F4,
    0x007B, 0x007B, 0x007B, 0x007B, 0x007B, 0xF8F3, 0x007C, 0xF8F2, 0xFF5B, 0xFE5B, 0xF8F1, 0xFE37,
    0x007D, 0x007D, 0x007D, 0x007D, 0x007D, 0xF8FE, 0x2016, 0xF8FD, 0xFF5D, 0xFE5C, 0xF8FC, 0xFE38,
    0x005B, 0x005B, 0x005B, 0x005B, 0x005B, 0xF8F0, 0xF8EF, 0xFF3B, 0xF8EE, 0x005D, 0x005D, 0x005D,
    0x005D, 0x005D, 0xF8FB, 0xF8FA, 0xFF3D, 0xF8F9, 0x02D8, 0x032E, 0x0306, 0x032F, 0x0311, 0x0361,
    0x032A, 0x033A, 0x00A6, 0x0180, 0xF6EA, 0x0183, 0x3076, 0x30D6, 0x2022, 0x25D8, 0x2219, 0x25CE,
    0x0063, 0x056E, 0x099A, 0x0107, 0x091A, 0x0A9A, 0x0A1A, 0x3388, 0x0981, 0x0310, 0x0901, 0x0A81,
    0xD809, 0x21EA, 0x2105, 0x02C7, 0x032C, 0x030C, 0x21B5, 0x3118, 0x010D, 0x00E7, 0x1E09, 0x24D2,
    0x0109, 0x0255, 0x010B, 0x010B, 0x33C5, 0x00B8, 0x0327, 0x2308, 0x2308, 0x2308, 0x2308, 0x2308,
    0x2309, 0x2309, 0x2309, 0x2309, 0x2309, 0x00A2, 0x2103, 0xF6DF, 0xFFE0, 0xF7A2, 0xF6E0, 0x0579,
    0x099B, 0x091B, 0x0A9B, 0x0A1B, 0x3114, 0x04BD, 0x2713, 0x0447, 0x04BF, 0x04B7, 0x04F5, 0x0573,
    0x04CC, 0x04B9, 0x03C7, 0x3277, 0x3217, 0x3269, 0x314A, 0x3209, 0x0E0A, 0x0E08, 0x0E09, 0x0E0C,
    0x0188, 0x3276, 0x3216, 0x3268, 0x3148, 0x3208, 0x321C, 0x25CB, 0x20DD, 0x2298, 0x2299, 0x2299,
    0x2299, 0x2296, 0x2297, 0x2297, 0x2297, 0x2299, 0x2295, 0x2295, 0x2295, 0x3036, 0x25D0, 0x25D1,
    0x02C6, 0x032D, 0x0302, 0x2327, 0x01C2, 0x01C0, 0x01C1, 0x01C3, 0x2663, 0x2663, 0x2667, 0x33A4,
    0xFF43, 0x33A0, 0x0581, 0x003A, 0x20A1, 0xFF1A, 0x20A1, 0xFE55, 0x02D1, 0x02D0, 0x002C, 0x0313,
    0x0315, 0xF6C3, 0x060C, 0x055D, 0xF6E1, 0xFF0C, 0x0314, 0x02BD, 0xFE50, 0xF6E2, 0x0312, 0x02BB,
    0x263C, 0x200C, 0x2245, 0x222E, 0x222E, 0x222E, 0x2303, 0x0006, 0x0007, 0x0008, 0x0018, 0x000D,
    0x0011, 0x0012, 0x0013, 0x0014, 0x007F, 0x0010, 0x0019, 0x0005, 0x0004, 0x001B, 0x0017, 0x0003,
    0x000C, 0x001C, 0x001D, 0x0009, 0x000A, 0x0015, 0x001E, 0x000F, 0x000E, 0x0002, 0x0001, 0x001A,
    0x0016, 0x001F, 0x000B, 0x2A3F, 0x2210, 0x2210, 0x00A9, 0xF8E9, 0xF6D9, 0x300C, 0xFF62, 0xFE41,
    0x300D, 0xFF63, 0xFE42, 0x337F, 0x33C7, 0x33C6, 0x249E, 0x20A2, 0x0297, 0x22CF, 0x22CE, 0x00A4,
    0x200C, 0xF6D1, 0xF6D2, 0xF6D4, 0xF6D5, 0x0064, 0x0564, 0x09A6, 0x0636, 0x0926, 0xFEBE, 0xFEBF,
    0xFEC0, 0x05BC, 0x05BC, 0x2020, 0x2021, 0x0AA6, 0x0A26, 0x3060, 0x30C0, 0x062F, 0x05D3, 0xFB33,
    0xFB33, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3,
    0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0x05D3, 0xFEAA, 0x064F,
    0x064F, 0x064C, 0x064C, 0x0964, 0x05A7, 0x05A7, 0x0485, 0x0111, 0xF6D3, 0x300A, 0xFE3D, 0x300B,
    0xFE3E, 0x032B, 0x21D4, 0x21D2, 0x27E6, 0x27E7, 0x0965, 0xF6D6, 0x030F, 0x222C, 0x2017, 0x0333,
    0x033F, 0x02BA, 0x2016, 0x030E, 0x3109, 0x33C8, 0x010F, 0x1E11, 0x24D3, 0x1E13, 0x0111, 0x09A1,
    0x0921, 0x0AA1, 0x0A21, 0x0688, 0xFB89, 0x095C, 0x09A2, 0x0922, 0x0AA2, 0x0A22, 0x1E0B, 0x1E0D,
    0x066B, 0x066B, 0x0434, 0x00B0, 0x05AD, 0x3067, 0x03EF, 0x30C7, 0x232B, 0x2326, 0x03B4, 0x018D,
    0x09F8, 0x02A4, 0x09A7, 0x0927, 0x0AA7, 0x0A27, 0x0257, 0x0385, 0x0344, 0x2662, 0x2666, 0x22C4,
    0x2662, 0x00A8, 0xF6D7, 0x0324, 0x0308, 0xF6D8, 0x0385, 0x3062, 0x30C2, 0x3003, 0x00F7, 0x2223,
    0x2215, 0x0452, 0x2593, 0x1E0F, 0x3397, 0x0111, 0xFF44, 0x2584, 0x0E0E, 0x0E14, 0x3069, 0x30C9,
    0x0024, 0xF6E3, 0xFF04, 0xF724, 0xFE69, 0xF6E4, 0x20AB, 0x3326, 0x02D9, 0x0307, 0x0323, 0x0323,
    0x30FB, 0x0131, 0x0237, 0xF6BE, 0x0284, 0x22C5, 0x25CC, 0xFB1F, 0xFB1F, 0x031E, 0x02D5, 0x249F,
    0xF6EB, 0x0256, 0x018C, 0x3065, 0x30C5, 0x01F3, 0x02A3, 0x01C6, 0x02A5, 0x04E1, 0x0455, 0x045F,
    0x0065, 0x00E9, 0x2641, 0x098F, 0x311C, 0x0115, 0x090D, 0x0A8D, 0x0945, 0x0AC5, 0x011B, 0x1E1D,
    0x0565, 0x0587, 0x24D4, 0x00EA, 0x1EBF, 0x1E19, 0x1EC7, 0x1EC1, 0x1EC3, 0x1EC5, 0x0454, 0x0205,
    0x090F, 0x00EB, 0x0117, 0x0117, 0x1EB9, 0x0A0F, 0x0A47, 0x0444, 0x00E8, 0x0A8F, 0x0567, 0x311D,
    0x3048, 0x1EBB, 0x311F, 0x0038, 0x0668, 0x09EE, 0x2467, 0x2791, 0x096E, 0x2471, 0x2485, 0x2499,
    0x0AEE, 0x0A6E, 0x0668, 0x3028, 0x266B, 0x3227, 0x2088, 0xFF18, 0xF738, 0x247B, 0x248F, 0x06F8,
    0x2177, 0x2078, 0x0E58, 0x0207, 0x0465, 0x30A8, 0xFF74, 0x0A74, 0x3154, 0x043B, 0x2208, 0x246A,
    0x247E, 0x2492, 0x217A, 0x2026, 0x22EE, 0x0113, 0x1E17, 0x1E15, 0x043C, 0x2014, 0xFE31, 0xFF45,
    0x055B, 0x2205, 0xD801, 0x3123, 0x043D, 0x2013, 0xFE32, 0x04A3, 0x014B, 0x3125, 0x04A5, 0x04C8,
    0x2002, 0x0119, 0x3153, 0x025B, 0x029A, 0x025C, 0x025E, 0x025D, 0x24A0, 0x03B5, 0x03F5, 0x03AD,
    0x003D, 0xFF1D, 0xFE66, 0x207C, 0x2261, 0x224D, 0x3126, 0x0440, 0x0258, 0x044D, 0x0441, 0x04AB,
    0x0283, 0x0286, 0x090E, 0x0946, 0x01AA, 0x0285, 0x3047, 0x30A7, 0xFF6A, 0x212E, 0xF6EC, 0x03B7,
    0x0568, 0x03AE, 0x00F0, 0x1EBD, 0x1E1B, 0x0591, 0x0591, 0x0591, 0x0591, 0x01DD, 0x3161, 0x20AC,
    0x09C7, 0x0947, 0x0AC7, 0x0021, 0x055C, 0x203C, 0x00A1, 0xF7A1, 0xFF01, 0xF721, 0x2203, 0x0292,
    0x01EF, 0x0293, 0x01B9, 0x01BA, 0x0066, 0xFB00, 0xFB03, 0xFB04, 0xFB01, 0xFB02, 0x095E, 0x0A5E,
    0x2109, 0x064E, 0x064E, 0x064B, 0x3108, 0x24D5, 0x1E1F, 0x0641, 0x0586, 0xFED2, 0xFED3, 0xFED4,
    0x03E5, 0x2640, 0xFB00, 0xFB03, 0xFB04, 0xFB01, 0x246E, 0x2482, 0x2496, 0x2012, 0x25A0, 0x25AC,
    0x05DA, 0xFB3A, 0xFB3A, 0x05DA, 0x05DA, 0x05DA, 0x05DA, 0x05DA, 0x05DD, 0x05DD, 0x05DF, 0x05DF,
    0x05E3, 0x05E3, 0x05E5, 0x05E5, 0x02C9, 0x25C9, 0x0473, 0x0035, 0x0665, 0x09EB, 0x2464, 0x278E,
    0x096B, 0x215D, 0x0AEB, 0x0A6B, 0x0665, 0x3025, 0x3224, 0x2085, 0xFF15, 0xF735, 0x2478, 0x248C,
    0x06F5, 0x2174, 0x2075, 0x0E55, 0xFB02, 0x266D, 0x230A, 0x230A, 0x230A, 0x230A, 0x230A, 0x230B,
    0x230B, 0x230B, 0x230B, 0x230B, 0x0192, 0xFF46, 0x3399, 0x0E1F, 0x0E1D, 0x227B, 0x227D, 0x0E4F,
    0x2200, 0x0034, 0x0664, 0x09EA, 0x2463, 0x278D, 0x096A, 0x0AEA, 0x0A6A, 0x0664, 0x3024, 0x3223,
    0x2084, 0xFF14, 0x09F7, 0xF734, 0x2477, 0x248B, 0x06F4, 0x2173, 0x2074, 0x246D, 0x2481, 0x2495,
    0x0E54, 0x02CB, 0x24A1, 0x2044, 0x20A3, 0x0067, 0x0997, 0x01F5, 0x0917, 0x06AF, 0xFB93, 0xFB94,
    0xFB95, 0x0A97, 0x0A17, 0x304C, 0x30AC, 0x03B3, 0x0263, 0x02E0, 0x03EB, 0x310D, 0x011F, 0x01E7,
    0x0123, 0x24D6, 0x011D, 0x0123, 0x0121, 0x0121, 0x0433, 0x3052, 0x30B2, 0x2251, 0x059C, 0x05F3,
    0x059D, 0x00DF, 0x059E, 0x05F4, 0x3013, 0x0998, 0x0572, 0x0918, 0x0A98, 0x0A18, 0x063A, 0xFECE,
    0xFECF, 0xFED0, 0x0495, 0x0493, 0x0491, 0x095A, 0x0A5A, 0x0260, 0x3393, 0x304E, 0x30AE, 0x0563,
    0x05D2, 0xFB32, 0xFB32, 0x05D2, 0x0453, 0x01BE, 0x0294, 0x0296, 0x02C0, 0x0295, 0x02C1, 0x02E4,
    0x02A1, 0x02A2, 0x1E21, 0xFF47, 0x3054, 0x30B4, 0x24A2, 0x33AC, 0x2207, 0x0060, 0x0316, 0x0300,
    0x0300, 0x0953, 0x02CE, 0xFF40, 0x0340, 0x003E, 0x2265, 0x22DB, 0xFF1E, 0x226B, 0x2273, 0x2277,
    0x2267, 0xFE65, 0x0261, 0x01E5, 0x3050, 0x00AB, 0x00BB, 0x2039, 0x203A, 0x30B0, 0x3318, 0x33C9,
    0x0068, 0x04A9, 0x06C1, 0x09B9, 0x04B3, 0x0939, 0x0AB9, 0x0A39, 0x062D, 0xFEA2, 0xFEA3, 0x306F,
    0xFEA4, 0x332A, 0x30CF, 0xFF8A, 0x0A4D, 0x0621, 0x0621, 0x0621, 0x0621, 0x0621, 0x0621, 0x0621,
    0x0621, 0x0621, 0x3164, 0x044A, 0x21BC, 0x21BD, 0x21BC, 0x21C0, 0x21C1, 0x21C0, 0x33CA, 0x05B2,
    0x05B2, 0x05B2, 0x05B2, 0x05B2, 0x05B2, 0x05B2, 0x05B2, 0x05B3, 0x05B3, 0x05B3, 0x05B3, 0x05B3,
    0x05B3, 0x05B3, 0x05B3, 0x05B1, 0x05B1, 0x05B1, 0x05B1, 0x05B1, 0x05B1, 0x05B1, 0x05B1, 0x0302,
    0x0302, 0x0302, 0x0127, 0x310F, 0x1E2B, 0x1E29, 0x24D7, 0x0125, 0x1E27, 0x1E23, 0x1E25, 0x05D4,
    0x2661, 0x2665, 0x2665, 0x2661, 0xFB34, 0xFB34, 0x06C1, 0x0647, 0x05D4, 0xFBA7, 0xFEEA, 0xFEEA,
    0xFBA5, 0xFBA4, 0xFBA8, 0xFEEB, 0x3078, 0xFBA9, 0xFEEC, 0x337B, 0x30D8, 0xFF8D, 0x3336, 0x0267,
    0x3339, 0x05D7, 0x05D7, 0x0266, 0x02B1, 0x327B, 0x321B, 0x326D, 0x314E, 0x320D, 0x3072, 0x30D2,
    0xFF8B, 0x05B4, 0x05B4, 0x05B4, 0x05B4, 0x05B4, 0x05B4, 0x05B4, 0x05B4, 0x1E96, 0xFF48, 0x0570,
    0x0E2B, 0x307B, 0x30DB, 0xFF8E, 0x05B9, 0x05B9, 0x05B9, 0x05B9, 0x05B9, 0x05B9, 0x05B9, 0x05B9,
    0x0E2E, 0x0309, 0x0309, 0x21A9, 0x0321, 0x0322, 0x21AA, 0x3342, 0x03E9, 0x2015, 0x031B, 0x2668,
    0x2302, 0x24A3, 0x02B0, 0x0265, 0x3075, 0x3333, 0x30D5, 0xFF8C, 0x02DD, 0x030B, 0x0195, 0x002D,
    0x2010, 0x002D, 0xF6E5, 0xFF0D, 0xFE63, 0xF6E6, 0x2010, 0x0069, 0x00ED, 0x044F, 0x0987, 0x3127,
    0x012D, 0x01D0, 0x24D8, 0x00EE, 0x0456, 0x0209, 0x328F, 0x328B, 0x323F, 0x323A, 0x32A5, 0x3006,
    0x3001, 0xFF64, 0x3237, 0x32A3, 0x322F, 0x323D, 0x329D, 0x3240, 0x3296, 0x3236, 0x322B, 0x3232,
    0x32A4, 0x3005, 0x3298, 0x3238, 0x32A7, 0x32A6, 0x32A9, 0x322E, 0x322A, 0x3234, 0x3002, 0x329E,
    0x3243, 0x3239, 0x323E, 0x32A8, 0x3299, 0x3242, 0x3233, 0x3000, 0x3235, 0x3231, 0x323B, 0x3230,
    0x323C, 0x322C, 0x322D, 0x3007, 0x328E, 0x328A, 0x3294, 0x3290, 0x328C, 0x328D, 0x0907, 0x00EF,
    0x1E2F, 0x04E5, 0x1ECB, 0x04D7, 0x0435, 0x3275, 0x3215, 0x3267, 0x3147, 0x3207, 0x00EC, 0x0A87,
    0x0A07, 0x3044, 0x1EC9, 0x0988, 0x0438, 0x0908, 0x0A88, 0x0A08, 0x0A40, 0x020B, 0x0439, 0x09C0,
    0x0940, 0x0AC0, 0x0133, 0x30A4, 0xFF72, 0x3163, 0x02DC, 0x05AC, 0x012B, 0x04E3, 0x2253, 0x0A3F,
    0xFF49, 0x2206, 0x221E, 0x056B, 0x222B, 0x2321, 0x2321, 0x222B, 0xF8F5, 0x222B, 0x2320, 0x2320,
    0x203D, 0xD80B, 0x2229, 0x22C2, 0x2293, 0x22C2, 0x3305, 0x25D8, 0x25D9, 0x263B, 0x0451, 0x012F,
    0x03B9, 0x03CA, 0x0390, 0x0269, 0x03AF, 0x24A4, 0x0A72, 0x3043, 0x30A3, 0xFF68, 0x09FA, 0x0268,
    0xF6ED, 0x309D, 0x30FD, 0x0129, 0x1E2D, 0x3129, 0x044E, 0x09BF, 0x093F, 0x0ABF, 0x0475, 0x0477,
    0x006A, 0x0571, 0x099C, 0x091C, 0x0A9C, 0x0A1C, 0x3110, 0x01F0, 0x24D9, 0x0135, 0x029D, 0x025F,
    0x0458, 0x062C, 0xFE9E, 0xFE9F, 0xFEA0, 0x0698, 0xFB8B, 0x099D, 0x091D, 0x0A9D, 0x0A1D, 0x057B,
    0x3004, 0xFF4A, 0x24A5, 0x02B2, 0x006B, 0x04A1, 0x0995, 0x1E31, 0x043A, 0x049B, 0x0915, 0x05DB,
    0x0643, 0xFB3B, 0xFB3B, 0xFEDA, 0x05DB, 0xFEDB, 0xFEDC, 0xFB4D, 0x0A95, 0x0A15, 0x304B, 0x04C4,
    0x30AB, 0xFF76, 0x03BA, 0x03F0, 0x3171, 0x3184, 0x3178, 0x3179, 0x330D, 0x0640, 0x0640, 0x30F5,
    0x3384, 0x0650, 0x064D, 0x049F, 0xFF70, 0x049D, 0x310E, 0x3389, 0x01E9, 0x0137, 0x24DA, 0x0137,
    0x1E33, 0x0584, 0x3051, 0x30B1, 0xFF79, 0x056F, 0x30F6, 0x0138, 0x0996, 0x0445, 0x0916, 0x0A96,
    0x0A16, 0x062E, 0xFEA6, 0xFEA7, 0xFEA8, 0x03E7, 0x0959, 0x0A59, 0x3278, 0x3218, 0x326A, 0x314B,
    0x320A, 0x0E02, 0x0E05, 0x0E03, 0x0E04, 0x0E5B, 0x0199, 0x0E06, 0x3391, 0x304D, 0x30AD, 0xFF77,
    0x3315, 0x3316, 0x3314, 0x326E, 0x320E, 0x3260, 0x3131, 0x3200, 0x3133, 0x045C, 0x1E35, 0x3398,
    0x33A6, 0xFF4B, 0x33A2, 0x3053, 0x33C0, 0x0E01, 0x30B3, 0xFF7A, 0x331E, 0x0481, 0x327F, 0x0343,
    0x24A6, 0x33AA, 0x046F, 0x33CF, 0x029E, 0x304F, 0x30AF, 0xFF78, 0x33B8, 0x33BE, 0x006C, 0x09B2,
    0x013A, 0x0932, 0x0AB2, 0x0A32, 0x0E45, 0xFEFC, 0xFEF8, 0xFEF7, 0xFEFA, 0xFEF9, 0xFEFB, 0xFEF6,
    0xFEF5, 0x0644, 0x03BB, 0x019B, 0x05DC, 0xFB3C, 0xFB3C, 0x05DC, 0x05DC, 0x05DC, 0x05DC, 0x05DC,
    0xFEDE, 0xFCCA, 0xFEDF, 0xFCC9, 0xFCCB, 0xFDF2, 0xFEE0, 0xFD88, 0xFCCC, 0xFEDF, 0xFEDF, 0x25EF,
    0x22A4, 0x019A, 0x026C, 0x310C, 0x013E, 0x013C, 0x24DB, 0x1E3D, 0x013C, 0x0140, 0x0140, 0x1E37,
    0x1E39, 0x031A, 0x0318, 0x003C, 0x2264, 0x22DA, 0xFF1C, 0x226A, 0x2272, 0x2276, 0x2266, 0xFE64,
    0x026E, 0x258C, 0x026D, 0x20A4, 0x056C, 0x01C9, 0x0459, 0xF6C0, 0x0933, 0x0AB3, 0x1E3B, 0x0934,
    0x09E1, 0x0961, 0x09E3, 0x0963, 0x026B, 0xFF4C, 0x33D0, 0x0E2C, 0x2227, 0x22C0, 0x22C0, 0x00AC,
    0x2310, 0x2228, 0x22C1, 0x22C1, 0x0E25, 0x017F, 0xFE4E, 0x0332, 0xFE4D, 0x25CA, 0x24A7, 0x2113,
    0x0142, 0x2113, 0xF6EE, 0x2591, 0x0E26, 0x098C, 0x090C, 0x09E2, 0x0962, 0x33D3, 0x006D, 0x09AE,
    0x00AF, 0x0331, 0x0304, 0x02CD, 0xFFE3, 0x1E3F, 0x092E, 0x0AAE, 0x0A2E, 0x05A4, 0x05A4, 0x307E,
    0xF895, 0xF894, 0x0E4B, 0xF893, 0xF88C, 0xF88B, 0x0E48, 0xF88A, 0xF884, 0x0E31, 0xF889, 0x0E47,
    0xF88F, 0xF88E, 0x0E49, 0xF88D, 0xF892, 0xF891, 0x0E4A, 0xF890, 0x0E46, 0x30DE, 0xFF8F, 0x2642,
    0x3347, 0x05BE, 0x2642, 0x05AF, 0x3383, 0x3107, 0x33D4, 0x24DC, 0x33A5, 0x1E41, 0x1E43, 0x0645,
    0xFEE2, 0xFEE3, 0xFEE4, 0xFCD1, 0xFC48, 0x334D, 0x3081, 0x337E, 0x30E1, 0xFF92, 0x05DE, 0xFB3E,
    0xFB3E, 0x05DE, 0x0574, 0x05A5, 0x05A6, 0x05A6, 0x05A5, 0x0271, 0x3392, 0xFF65, 0x00B7, 0x3272,
    0x3212, 0x3264, 0x3141, 0x3170, 0x3204, 0x316E, 0x316F, 0x307F, 0x30DF, 0xFF90, 0x2212, 0x0320,
    0x2296, 0x02D7, 0x2213, 0x2032, 0x334A, 0x3349, 0x0270, 0x3396, 0x33A3, 0xFF4D, 0x339F, 0x3082,
    0x33C1, 0x30E2, 0xFF93, 0x33D6, 0x0E21, 0x33A7, 0x33A8, 0x24A8, 0x33AB, 0x33B3, 0xF6EF, 0x026F,
    0x00B5, 0x00B5, 0x3382, 0x226B, 0x226A, 0x338C, 0x03BC, 0x338D, 0x3080, 0x30E0, 0xFF91, 0x3395,
    0x00D7, 0x339B, 0x05A3, 0x05A3, 0x266A, 0x266B, 0x266D, 0x266F, 0x33B2, 0x33B6, 0x33BC, 0x33B9,
    0x33B7, 0x33BF, 0x33BD, 0x006E, 0x09A8, 0x2207, 0x0144, 0x0928, 0x0AA8, 0x0A28, 0x306A, 0x30CA,
    0xFF85, 0x0149, 0x3381, 0x266E, 0x310B, 0x00A0, 0x0148, 0x0146, 0x24DD, 0x1E4B, 0x0146, 0x1E45,
    0x1E47, 0x0338, 0x306D, 0x30CD, 0xFF88, 0x20AA, 0x338B, 0x014B, 0x0999, 0x0919, 0x0A99, 0x0A19,
    0x0E07, 0x3093, 0x0272, 0x0273, 0x326F, 0x320F, 0x3135, 0x3261, 0x3136, 0x3134, 0x3168, 0x3201,
    0x3167, 0x3166, 0x306B, 0x30CB, 0xFF86, 0xF899, 0x0E4D, 0x0039, 0x0669, 0x09EF, 0x2468, 0x2792,
    0x096F, 0x0AEF, 0x0A6F, 0x0669, 0x3029, 0x3228, 0x2089, 0xFF19, 0xF739, 0x247C, 0x2490, 0x06F9,
    0x2178, 0x2079, 0x2472, 0x2486, 0x249A, 0x0E59, 0x01CC, 0x045A, 0x30F3, 0xFF9D, 0x019E, 0x1E49,
    0xFF4E, 0x339A, 0x09A3, 0x0923, 0x0AA3, 0x0A23, 0x0929, 0x306E, 0x30CE, 0xFF89, 0x00A0, 0x0E13,
    0x0E19, 0x0646, 0xFEE6, 0x06BA, 0xFB9F, 0xFEE7, 0xFEE7, 0xFCD2, 0xFC4B, 0xFEE8, 0xFCD5, 0xFC4E,
    0xFC8D, 0x220C, 0x2209, 0x2209, 0x2260, 0x226F, 0x2271, 0x2279, 0x2262, 0x226E, 0x2270, 0x2226,
    0x2280, 0x2284, 0x2281, 0x2285, 0x0576, 0x24A9, 0x33B1, 0x207F, 0x00F1, 0x03BD, 0x306C, 0x30CC,
    0xFF87, 0x09BC, 0x093C, 0x0ABC, 0x0A3C, 0x0023, 0xFF03, 0xFE5F, 0x0374, 0x0375, 0x2116, 0x05E0,
    0xFB40, 0xFB40, 0x05E0, 0x33B5, 0x33BB, 0x099E, 0x091E, 0x0A9E, 0x0A1E, 0x006F, 0x00F3, 0x0E2D,
    0x0275, 0x04E9, 0x04EB, 0x0993, 0x311B, 0x014F, 0x0911, 0x0A91, 0x0949, 0x0AC9, 0x01D2, 0x24DE,
    0x00F4, 0x1ED1, 0x1ED9, 0x1ED3, 0x1ED5, 0x1ED7, 0x043E, 0x0151, 0x020D, 0x0913, 0x00F6, 0x04E7,
    0x1ECD, 0x0153, 0x315A, 0x02DB, 0x0328, 0x00F2, 0x0A93, 0x0585, 0x304A, 0x1ECF, 0x01A1, 0x1EDB,
    0x1EE3, 0x1EDD, 0x1EDF, 0x1EE1, 0x0151, 0x01A3, 0x020F, 0x30AA, 0xFF75, 0x3157, 0x05AB, 0x014D,
    0x1E53, 0x1E51, 0x0950, 0x03C9, 0x03D6, 0x0461, 0x0277, 0x047B, 0x047D, 0x03CE, 0x0AD0, 0x03BF,
    0x03CC, 0xFF4F, 0x0031, 0x0661, 0x09E7, 0x2460, 0x278A, 0x0967, 0x2024, 0x215B, 0xF6DC, 0x0AE7,
    0x0A67, 0x0661, 0x00BD, 0x3021, 0x3220, 0x2081, 0xFF11, 0x09F4, 0xF731, 0x2474, 0x2488, 0x06F1,
    0x00BC, 0x2170, 0x00B9, 0x0E51, 0x2153, 0x01EB, 0x01ED, 0x0A13, 0x0A4B, 0x0254, 0x24AA, 0x25E6,
    0x2325, 0x00AA, 0x00BA, 0x221F, 0x0912, 0x094A, 0x00F8, 0x01FF, 0x3049, 0x30A9, 0xFF6B, 0x01FF,
    0xF6F0, 0x047F, 0x00F5, 0x1E4D, 0x1E4F, 0x3121, 0x203E, 0xFE4A, 0x0305, 0xFE49, 0xFE4C, 0xFE4B,
    0x00AF, 0x09CB, 0x094B, 0x0ACB, 0x220B, 0x0070, 0x3380, 0x332B, 0x09AA, 0x1E55, 0x092A, 0x21DF,
    0x21DE, 0x0AAA, 0x0A2A, 0x3071, 0x0E2F, 0x30D1, 0x0484, 0x04C0, 0x317F, 0x00B6, 0x2225, 0x0028,
    0x0028, 0x0028, 0xFD3E, 0x0028, 0x0028, 0xF8ED, 0x007C, 0xF8EC, 0x208D, 0xFF08, 0xFE59, 0x207D,
    0xF8EB, 0xFE35, 0x0029, 0x0029, 0x0029, 0xFD3F, 0x0029, 0x0029, 0xF8F8, 0x007C, 0xF8F7, 0x208E,
    0xFF09, 0xFE5A, 0x207E, 0xF8F6, 0xFE36, 0x2202, 0x05C0, 0x0599, 0x33A9, 0x05B7, 0x05B7, 0x05B7,
    0x05B7, 0x05B7, 0x05B7, 0x05B7, 0x05B7, 0x05A1, 0x3106, 0x24DF, 0x1E57, 0x05E4, 0x043F, 0xFB44,
    0xFB44, 0x333B, 0xFB43, 0x067E, 0x057A, 0x05E4, 0xFB57, 0xFB58, 0x307A, 0xFB59, 0x30DA, 0x04A7,
    0xFB4E, 0x0025, 0x066A, 0xFF05, 0xFE6A, 0x002E, 0x0589, 0x00B7, 0xFF61, 0xF6E7, 0xFF0E, 0xFE52,
    0xF6E8, 0x0342, 0x22A5, 0x2031, 0x2030, 0x20A7, 0x338A, 0x09AB, 0x092B, 0x0AAB, 0x0A2B, 0x03C6,
    0x03D5, 0x327A, 0x321A, 0x326C, 0x314D, 0x320C, 0x0278, 0x0E3A, 0x03D5, 0x01A5, 0x0E1E, 0x0E1C,
    0x0E20, 0x03C0, 0x03D6, 0x3273, 0x3213, 0x3176, 0x3265, 0x3172, 0x3142, 0x3205, 0x3174, 0x3144,
    0x3175, 0x3177, 0x3173, 0x3074, 0x30D4, 0x03D6, 0x0583, 0x002B, 0x031F, 0x2295, 0x00B1, 0x02D6,
    0xFF0B, 0xFE62, 0x207A, 0xFF50, 0x33D8, 0x307D, 0x261F, 0x261C, 0x261E, 0x261D, 0x30DD, 0x0E1B,
    0x3012, 0x3020, 0x24AB, 0x227A, 0x227C, 0x211E, 0x2032, 0x02B9, 0x2035, 0x220F, 0x220F, 0x220F,
    0x2305, 0x30FC, 0x2318, 0x2282, 0x2283, 0x2237, 0x221D, 0x03C8, 0x0471, 0x0486, 0x33B0, 0x3077,
    0x30D7, 0x2014, 0x33B4, 0x33BA, 0x0071, 0x0958, 0x05A8, 0x0642, 0xFED6, 0xFED7, 0xFED8, 0x05B8,
    0x05B8, 0x05B8, 0x05B8, 0x05B8, 0x05B8, 0x05B8, 0x05B8, 0x05B8, 0x05B8, 0x05B8, 0x05B8, 0x05B8,
    0x05B8, 0x05B8, 0x05B8, 0x059F, 0x3111, 0x24E0, 0x02A0, 0xFF51, 0x05E7, 0xFB47, 0xFB47, 0x05E7,
    0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x05E7,
    0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x05E7, 0x24AC, 0x2669, 0x05BB, 0x05BB,
    0x05BB, 0x05BB, 0x05BB, 0x05BB, 0x05BB, 0x05BB, 0x003F, 0x061F, 0x055E, 0x00BF, 0xF7BF, 0x037E,
    0xFF1F, 0xF73F, 0x0022, 0x201E, 0x201C, 0xFF02, 0x301E, 0x301D, 0x201D, 0x2018, 0x201B, 0x201B,
    0x2019, 0x0149, 0x201A, 0x0027, 0xFF07, 0x0072, 0x057C, 0x09B0, 0x0155, 0x0930, 0x221A, 0x221A,
    0x221A, 0x221A, 0x221A, 0x221A, 0xF8E5, 0x33AE, 0x33AF, 0x33AD, 0x05BF, 0x05BF, 0x0AB0, 0x0A30,
    0x3089, 0x30E9, 0xFF97, 0x09F1, 0x09F0, 0x0264, 0x2013, 0x2236, 0x3116, 0x0159, 0x0157, 0x24E1,
    0x0157, 0x0211, 0x1E59, 0x1E5B, 0x1E5D, 0x203B, 0x2286, 0x2287, 0x00AE, 0xF8E8, 0xF6DA, 0x0631,
    0x0580, 0xFEAE, 0x308C, 0x0631, 0x30EC, 0xFF9A, 0x05E8, 0xFB48, 0x05E8, 0x05E8, 0x05E8, 0x05E8,
    0x05E8, 0x05E8, 0x05E8, 0x05E8, 0x05E8, 0x05E8, 0x05E8, 0x05E8, 0x05E8, 0x05E8, 0x05E8, 0x05E8,
    0x05E8, 0x05E8, 0x05E8, 0x05E8, 0x05E8, 0x223D, 0x0597, 0x0597, 0x2310, 0x027E, 0x027F, 0x09DD,
    0x095D, 0x03C1, 0x03F1, 0x027D, 0x027B, 0x02B5, 0x03F1, 0x02DE, 0x3271, 0x3211, 0x3263, 0x3140,
    0x313A, 0x3169, 0x3139, 0x313B, 0x316C, 0x3203, 0x313F, 0x313C, 0x316B, 0x313D, 0x313E, 0x316A,
    0x316D, 0x221F, 0x0319, 0x22BF, 0x308A, 0x30EA, 0xFF98, 0x02DA, 0x0325, 0x030A, 0x02BF, 0x0559,
    0x031C, 0x02D3, 0x02BE, 0x0339, 0x02D2, 0x0213, 0x3351, 0x1E5F, 0x027C, 0x027A, 0xFF52, 0x308D,
    0x30ED, 0xFF9B, 0x0E23, 0x24AD, 0x09DC, 0x0931, 0x0A5C, 0x0691, 0xFB8D, 0x09E0, 0x0960, 0x0AE0,
    0x09C4, 0x0944, 0x0AC4, 0xF6F1, 0x2590, 0x0279, 0x02B4, 0x308B, 0x30EB, 0xFF99, 0x09F2, 0x09F3,
    0xF6DD, 0x0E24, 0x098B, 0x090B, 0x0A8B, 0x09C3, 0x0943, 0x0AC3, 0x0073, 0x09B8, 0x015B, 0x1E65,
    0x0635, 0x0938, 0xFEBA, 0xFEBB, 0xFEBC, 0x0AB8, 0x0A38, 0x3055, 0x30B5, 0xFF7B, 0xFDFA, 0x05E1,
    0xFB41, 0xFB41, 0x05E1, 0x0E32, 0x0E41, 0x0E44, 0x0E43, 0x0E33, 0x0E30, 0x0E40, 0xF886, 0x0E35,
    0xF885, 0x0E34, 0x0E42, 0xF888, 0x0E37, 0xF887, 0x0E36, 0x0E38, 0x0E39, 0x3119, 0x0161, 0x1E67,
    0x015F, 0x0259, 0x04D9, 0x04DB, 0x025A, 0x24E2, 0x015D, 0x0219, 0x1E61, 0x1E63, 0x1E69, 0x033C,
    0x2033, 0x02CA, 0x00A7, 0x0633, 0xFEB2, 0xFEB3, 0xFEB4, 0x05B6, 0x05B6, 0x05B6, 0x05B6, 0x05B6,
    0x05B6, 0x05B6, 0x0592, 0x05B6, 0x057D, 0x305B, 0x30BB, 0xFF7E, 0x003B, 0x061B, 0xFF1B, 0xFE54,
    0x309C, 0xFF9F, 0x3322, 0x3323, 0x0037, 0x0667, 0x09ED, 0x2466, 0x2790, 0x096D, 0x215E, 0x0AED,
    0x0A6D, 0x0667, 0x3027, 0x3226, 0x2087, 0xFF17, 0xF737, 0x247A, 0x248E, 0x06F7, 0x2176, 0x2077,
    0x2470, 0x2484, 0x2498, 0x0E57, 0x00AD, 0x0577, 0x09B6, 0x0448, 0x0651, 0xFC61, 0xFC5E, 0xFC60,
    0x0651, 0xFC62, 0xFC5F, 0x2592, 0x2593, 0x2591, 0x2592, 0x0936, 0x0AB6, 0x0A36, 0x0593, 0x266F,
    0x3115, 0x0449, 0x0634, 0xFEB6, 0xFEB7, 0xFEB8, 0x03E3, 0x20AA, 0x20AA, 0x05B0, 0x05B0, 0x05B0,
    0x05B0, 0x05B0, 0x05B0, 0x05B0, 0x05B0, 0x05B0, 0x04BB, 0x03ED, 0x05E9, 0xFB49, 0xFB49, 0xFB2C,
    0xFB2C, 0xFB2D, 0xFB2D, 0x05C1, 0x05E9, 0xFB2A, 0xFB2A, 0xFB2B, 0xFB2B, 0x0282, 0x03C3, 0x03C2,
    0x03C2, 0x03F2, 0x3057, 0x30B7, 0xFF7C, 0x05BD, 0x05BD, 0x223C, 0x2243, 0x05C2, 0x3274, 0x3214,
    0x317E, 0x3266, 0x317A, 0x3145, 0x317B, 0x3206, 0x317D, 0x317C, 0x0036, 0x0666, 0x09EC, 0x2465,
    0x278F, 0x096C, 0x0AEC, 0x0A6C, 0x0666, 0x3026, 0x3225, 0x2086, 0xFF16, 0xF736, 0x2479, 0x248D,
    0x06F6, 0x2175, 0x2076, 0x246F, 0x09F9, 0x2483, 0x2497, 0x0E56, 0x002F, 0x2215, 0x2215, 0x2215,
    0x2215, 0xFF0F, 0x017F, 0x1E9B, 0x2322, 0x2323, 0x263A, 0xFF53, 0x05C3, 0x00AD, 0x044C, 0x305D,
    0x30BD, 0xFF7F, 0x0338, 0x0337, 0x0E29, 0x0E28, 0x0E0B, 0x0E2A, 0x0020, 0x0020, 0x2660, 0x2660,
    0x2664, 0x24AE, 0x033B, 0x33C4, 0x339D, 0x25A9, 0x25A4, 0x338F, 0x339E, 0x33CE, 0x33D1, 0x33D2,
    0x338E, 0x33D5, 0x339C, 0x33A1, 0x25A6, 0x25A7, 0x25A8, 0x25A5, 0x25A3, 0x33DB, 0x09B7, 0x0937,
    0x0AB7, 0x3149, 0x3185, 0x3180, 0x3132, 0x3165, 0x3143, 0x3146, 0x3138, 0xF6F2, 0x22C6, 0x00A3,
    0xFFE1, 0x0336, 0x0335, 0x2282, 0x228A, 0x2286, 0x2291, 0x227B, 0x220B, 0x3059, 0x30B9, 0xFF7D,
    0x0652, 0x2211, 0x2211, 0x2211, 0x263C, 0x2283, 0x228B, 0x2287, 0x2292, 0x33DC, 0x337C, 0x0074,
    0x09A4, 0x22A4, 0x22A3, 0x0924, 0x0AA4, 0x0A24, 0x0637, 0xFEC2, 0xFEC3, 0x305F, 0xFEC4, 0x337D,
    0x30BF, 0xFF80, 0x0640, 0x03C4, 0x05EA, 0xFB4A, 0xFB4A, 0xFB4A, 0x05EA, 0x0167, 0x310A, 0x0165,
    0x02A8, 0x0163, 0x0686, 0xFB7B, 0xFB7C, 0xFB7D, 0xFB7C, 0x24E3, 0x1E71, 0x0163, 0x1E97, 0x1E6B,
    0x1E6D, 0x0442, 0x04AD, 0x062A, 0xFE96, 0xFCA2, 0xFC0C, 0xFE97, 0x3066, 0xFCA1, 0xFC0B, 0x0629,
    0xFE94, 0xFE98, 0xFCA4, 0xFC0E, 0xFC73, 0x30C6, 0xFF83, 0x2121, 0x260E, 0x05A0, 0x05A9, 0x2469,
    0x3229, 0x247D, 0x2491, 0x2179, 0x02A7, 0x05D8, 0xFB38, 0xFB38, 0x05D8, 0x04B5, 0x059B, 0x059B,
    0x09A5, 0x0925, 0x0AA5, 0x0A25, 0x0630, 0xFEAC, 0xF898, 0xF897, 0x0E4C, 0xF896, 0x062B, 0xFE9A,
    0xFE9B, 0xFE9C, 0x2203, 0x2234, 0x03B8, 0x03D1, 0x03D1, 0x3279, 0x3219, 0x326B, 0x314C, 0x320B,
    0x246C, 0x2480, 0x2494, 0x0E11, 0x01AD, 0x0E12, 0x00FE, 0x0E17, 0x0E10, 0x0E18, 0x0E16, 0x0482,
    0x066C, 0x066C, 0x0033, 0x0663, 0x09E9, 0x2462, 0x278C, 0x0969, 0x215C, 0x0AE9, 0x0A69, 0x0663,
    0x3023, 0x3222, 0x2083, 0xFF13, 0x09F6, 0xF733, 0x2476, 0x248A, 0x06F3, 0x00BE, 0xF6DE, 0x2172,
    0x00B3, 0x0E53, 0x3394, 0x2040, 0x3061, 0x30C1, 0xFF81, 0x3270, 0x3210, 0x3262, 0x3137, 0x3202,
    0x02DC, 0x0330, 0x0303, 0x0303, 0x0360, 0x223C, 0x0334, 0x033E, 0x0303, 0x0303, 0x0303, 0x2297,
    0x0596, 0x0596, 0x0A70, 0x0483, 0x057F, 0x1E6F, 0xFF54, 0x0569, 0x3068, 0x30C8, 0xFF84, 0x02E5,
    0x02E9, 0x02E6, 0x02E8, 0x02E7, 0x01BD, 0x0185, 0x01A8, 0x0384, 0x3327, 0x0E0F, 0x3014, 0xFE5D,
    0xFE39, 0x3015, 0xFE5E, 0xFE3A, 0x0E15, 0x01AB, 0x24AF, 0x2122, 0xF8EA, 0xF6DB, 0x0288, 0x25BC,
    0x25C4, 0x25BA, 0x25B2, 0x25B3, 0x25BD, 0x25B9, 0x25C3, 0x02A6, 0x05E6, 0xFB46, 0xFB46, 0x05E6,
    0x0446, 0x05B5, 0x05B5, 0x05B5, 0x05B5, 0x05B5, 0x05B5, 0x05B5, 0x05B5, 0x045B, 0xF6F3, 0x099F,
    0x091F, 0x0A9F, 0x0A1F, 0x0679, 0xFB67, 0xFB68, 0xFB69, 0x09A0, 0x0920, 0x0AA0, 0x0A20, 0x0287,
    0x3064, 0x30C4, 0xFF82, 0x22A2, 0x22A3, 0x3063, 0x30C3, 0xFF6F, 0x246B, 0x247F, 0x2493, 0x217B,
    0xF6DE, 0x2473, 0x5344, 0x2487, 0x249B, 0x0032, 0x0662, 0x09E8, 0x2461, 0x278B, 0x0968, 0x2025,
    0x2025, 0xFE30, 0x0AE8, 0x0A68, 0x0662, 0x3022, 0x3221, 0x2082, 0xFF12, 0x09F5, 0xF732, 0x2475,
    0x2489, 0x06F2, 0x2171, 0x01BB, 0x00B2, 0x0E52, 0x2154, 0x0075, 0x00FA, 0x0289, 0x0989, 0x3128,
    0x016D, 0x01D4, 0x24E4, 0x00FB, 0x1E77, 0x0443, 0x0951, 0x0171, 0x0215, 0x0909, 0x00FC, 0x01D8,
    0x1E73, 0x01DA, 0x04F1, 0x01DC, 0x01D6, 0x1EE5, 0x00F9, 0x0A89, 0x0A09, 0x3046, 0x1EE7, 0x01B0,
    0x1EE9, 0x1EF1, 0x1EEB, 0x1EED, 0x1EEF, 0x0171, 0x04F3, 0x0217, 0x30A6, 0xFF73, 0x0479, 0x315C,
    0x016B, 0x04EF, 0x1E7B, 0x0A41, 0xFF55, 0x005F, 0x2017, 0xFF3F, 0xFE33, 0xFE4F, 0x222A, 0x22C3,
    0x228E, 0x228E, 0x228E, 0x2294, 0x2294, 0x2294, 0x22C3, 0x2200, 0x0173, 0x24B0, 0x2580, 0x05C4,
    0x03C5, 0x03CB, 0x03B0, 0x028A, 0x03CD, 0x031D, 0x02D4, 0x0A73, 0x016F, 0x045E, 0x3045, 0x30A5,
    0xFF69, 0x04AF, 0x04B1, 0x0169, 0x1E79, 0x1E75, 0x098A, 0x090A, 0x0A8A, 0x0A0A, 0x0A42, 0x09C2,
    0x0942, 0x0AC2, 0x09C1, 0x0941, 0x0AC1, 0x0076, 0x0935, 0x0AB5, 0x0A35, 0x30F7, 0x05D5, 0xFB35,
    0xFB35, 0xFB35, 0x05D5, 0xFB4B, 0xFB4B, 0x05F0, 0x05F1, 0x24E5, 0x1E7F, 0x20D7, 0x0432, 0x06A4,
    0xFB6B, 0xFB6C, 0xFB6D, 0x30F9, 0x2640, 0x007C, 0x030D, 0x0329, 0x02CC, 0x02C8, 0x057E, 0x028B,
    0x30F8, 0x09CD, 0x094D, 0x0ACD, 0x0983, 0x0903, 0x0A83, 0x2423, 0x2423, 0xFF56, 0x0578, 0x309E,
    0x30FE, 0x309B, 0xFF9E, 0x30FA, 0x24B1, 0x1E7D, 0x028C, 0x3094, 0x30F4, 0x0077, 0x1E83, 0x3159,
    0x308F, 0x30EF, 0xFF9C, 0x3158, 0x308E, 0x30EE, 0x3357, 0x301C, 0xFE34, 0x0648, 0xFEEE, 0x0624,
    0xFE86, 0x33DD, 0x24E6, 0x0175, 0x1E85, 0x1E87, 0x1E89, 0x3091, 0x2118, 0x30F1, 0x315E, 0x315D,
    0x1E81, 0x25E6, 0x25CB, 0x25D9, 0x300E, 0xFE43, 0x300F, 0xFE44, 0x25C7, 0x25C8, 0x25BF, 0x25BD,
    0x25C3, 0x25C1, 0x3016, 0x3017, 0x25B9, 0x25B7, 0x25AB, 0x263A, 0x25A1, 0x2606, 0x260F, 0x3018,
    0x3019, 0x25B5, 0x25B3, 0x3090, 0x30F0, 0x315F, 0xFF57, 0x3092, 0x30F2, 0xFF66, 0x20A9, 0xFFE6,
    0x0E27, 0x24B2, 0x2240, 0x1E98, 0x02B7, 0x028D, 0x01BF, 0x0078, 0x033D, 0x3112, 0x24E7, 0x1E8D,
    0x1E8B, 0x056D, 0x03BE, 0xFF58, 0x24B3, 0x02E3, 0x0079, 0x334E, 0x09AF, 0x00FD, 0x092F, 0x3152,
    0x0AAF, 0x0A2F, 0x3084, 0x30E4, 0xFF94, 0x3151, 0x0E4E, 0x3083, 0x30E3, 0xFF6C, 0x0463, 0x24E8,
    0x0177, 0x00FF, 0x1E8F, 0x1EF5, 0x064A, 0x06D2, 0xFBAF, 0xFEF2, 0x0626, 0xFE8A, 0xFE8B, 0xFE8C,
    0xFEF3, 0xFEF4, 0xFCDD, 0xFC58, 0xFC94, 0x06D1, 0x3156, 0x00A5, 0xFFE5, 0x3155, 0x3186, 0x05AA,
    0x05AA, 0x044B, 0x04F9, 0x3181, 0x3183, 0x3182, 0x059A, 0x1EF3, 0x01B4, 0x1EF7, 0x0575, 0x0457,
    0x3162, 0x262F, 0x0582, 0xFF59, 0x05D9, 0xFB39, 0xFB39, 0x05D9, 0x05F2, 0xFB1F, 0x3088, 0x3189,
    0x30E8, 0xFF96, 0x315B, 0x3087, 0x30E7, 0xFF6E, 0x03F3, 0x3188, 0x3187, 0x0E22, 0x0E0D, 0x24B4,
    0x037A, 0x0345, 0x01A6, 0x1E99, 0x02B8, 0x1EF9, 0x028E, 0x3086, 0x318C, 0x30E6, 0xFF95, 0x3160,
    0x046B, 0x046D, 0x0467, 0x0469, 0x3085, 0x30E5, 0xFF6D, 0x318B, 0x318A, 0x09DF, 0x095F, 0x007A,
    0x0566, 0x017A, 0x095B, 0x0A5B, 0x0638, 0xFEC6, 0xFEC7, 0x3056, 0xFEC8, 0x0632, 0xFEB0, 0x30B6,
    0x0595, 0x0594, 0x0598, 0x05D6, 0xFB36, 0xFB36, 0x05D6, 0x3117, 0x017E, 0x24E9, 0x1E91, 0x0291,
    0x017C, 0x017C, 0x1E93, 0x0437, 0x0499, 0x04DF, 0x305C, 0x30BC, 0x0030, 0x0660, 0x09E6, 0x0966,
    0x0AE6, 0x0A66, 0x0660, 0x2080, 0xFF10, 0xF730, 0x06F0, 0x2070, 0x0E50, 0xFEFF, 0x200C, 0x200B,
    0x03B6, 0x3113, 0x056A, 0x04C2, 0x0436, 0x0497, 0x04DD, 0x3058, 0x30B8, 0x05AE, 0x1E95, 0xFF5A,
    0x305E, 0x30BE, 0x24B5, 0x0290, 0x01B6, 0x305A, 0x30BA,
];

fn name_at(i: usize) -> &'static str {
    let start = OFFSETS[i] as usize;
    let end = NAMES[start..]
        .find('\n')
        .map(|n| start + n)
        .unwrap_or(NAMES.len());
    &NAMES[start..end]
}

/// The Unicode value the Adobe Glyph List gives a glyph name.
pub(crate) fn unicode_of(name: &str) -> Option<u16> {
    let (mut lo, mut hi) = (0, OFFSETS.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        match name_at(mid).cmp(name) {
            std::cmp::Ordering::Equal => return Some(CODES[mid]),
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
        }
    }
    None
}
