# Apple Visual Language for Boot and Failure Screens

Research as of November 2026. Every fact below cites a fetched source. Items from search snippets or secondary sources are labelled. Apple's HIG colour values are quoted as published and should be treated as reference only (Apple says they may change between releases).

## Key Question 1: macOS boot splash, progress bar and Startup Manager

### Takeaway
Apple's own documentation describes the boot sequence only in functional terms: an Apple logo when a startup disk is found, then a progress bar with or without the logo. No primary Apple source gives timing, animation curves or exact colours for the boot screen. The black-with-white-logo look dates to Yosemite (10.10), per secondary sources.

### Cited Findings
- Apple: "Your Mac shows an Apple logo when it finds your built-in or external startup disk." and "As startup continues, you should see a progress bar, with or without the Apple logo." It also says "This screen might alternate with a blank screen several times." — [Apple Support, If your Mac starts up to an Apple logo or progress bar](https://support.apple.com/en-us/HT207019)
- Startup Manager (Intel Macs): holding Option "Start up to Startup Manager, which allows you to choose other available startup disks or volumes." — [Apple Support, Mac startup key combinations](https://support.apple.com/en-us/102603)
- Apple silicon: "Release the power button when you see the startup options screen," which Apple describes as "a gear icon labeled Options." — [Apple Support, Mac startup key combinations](https://support.apple.com/en-us/102603)
- Pre-Yosemite OS X showed a grey background with a grey Apple logo and later a loading circle. Yosemite replaced these with a black screen, white Apple logo and a progress bar instead of the spinner. Secondary source, approximate: [Avid wiki, OS X](https://avid.wiki/OS_X). Forum users report the look depended on model and whether the OS was upgraded or shipped with Yosemite, and attribute this to an EFI update: [MacRumors forum, Yosemite boot screen](https://forums.macrumors.com/threads/1745984). User accounts, not authoritative.
- Forum reports describe Yosemite's change as a design choice and some users disliked it. No Apple rationale was found. [MacRumors forum, Yosemite restart screen progress bar](https://forums.macrumors.com/threads/yosemite-restart-screen-progress-bar.1802350) (secondary, anecdotal)

### Inferences
- The minimal pattern is: one centred mark, one thin determinate or indeterminate bar, nothing else. Apple's support description matches this structure, so the "one focal element plus progress" model is well grounded even though the exact pixel values are not.
- Startup Manager is a simple list of bootable volumes. Apple's own page gives no visual spec for it, so any design for a hobby OS here is a reconstruction.
- Black background with white mark: consistent with the Yosemite-era description above. The exact black value is not sourced (see Gaps).

### Gaps
- No Apple source gives boot-splash timing, fade durations, progress-bar height, or the exact background colour. Approximate only: black background, white mark, thin bar, as described by secondary sources.
- No Apple source describes the Startup Manager visual layout.
- Apple's support pages do not say whether the black or grey background is a deliberate design token or a firmware effect.

---

## Key Question 2: Kernel panic and "restarted because of a problem" screens

### Takeaway
Apple moved from an on-screen, multilingual, grey-curtain panic message (10.2 to 10.7) to an automatic restart followed by a short, plain message, with technical logs sent after login. The modern message is a single sentence with no technical data. Note that sources disagree on the exact sequence after 10.8.

### Cited Findings
- Mac OS X 10.2 to 10.7: "a transparent grey curtain dropped down the display" revealing a multi-language message telling the user to restart. The article's image is a composite overlay, not a true screenshot. [Eclectic Light, A brief history of kernel panics](https://eclecticlight.co/2024/07/27/a-brief-history-of-kernel-panics/)
- Per Wikipedia, 10.2 used "a dark translucent overlay" with "a white-background notice" in English, French, German and Japanese. 10.3 to 10.5 used a dark grey notice background. 10.6 to 10.7 revised the wording and added Spanish. In 10.2 and later the text sat on a standby (power) symbol rather than filling the screen. [Wikipedia, Kernel panic](https://en.wikipedia.org/wiki/Kernel_panic)
  - Conflict: Eclectic Light says "grey curtain" for 10.2; Wikipedia says white-background notice on a dark overlay for 10.2. Treat exact 10.2 colours as unresolved.
- Mountain Lion (10.8): "the panic display disappeared," users learned of the panic only after an automatic restart, and "Apple called these 'unexpected restarts'". The multilingual message was removed. A panic alert then appeared after login, "containing the panic log," and offered to send it to Apple. [Eclectic Light](https://eclecticlight.co/2024/07/27/a-brief-history-of-kernel-panics/)
  - Conflict: Wikipedia says 10.8 and later show "a message for a few seconds informing the user that a problem caused the computer to restart" after reboot, including a Chinese translation. Eclectic Light says the multilingual message was removed. The two sources describe different post-restart behaviour, possibly by version.
- Eclectic Light comment (unverified, Sonoma, Intel iMac): the message "Your computer restarted because of a problem" was displayed "on top of a giant 'power button' graphic for a couple seconds." [Eclectic Light, comments](https://eclecticlight.co/2024/07/27/a-brief-history-of-kernel-panics/)
- Current Apple support text: "Rarely, your Mac might experience a software or hardware issue that requires it to restart. When it starts up again, you might see a message that your computer was restarted because of a problem." Advice given is for the user: install updates, try safe mode, reconnect peripherals one at a time, run Apple Diagnostics. [Apple Support, If your Mac restarted because of a problem](https://support.apple.com/en-us/102382)
- Apple's support article does not describe the dialog's colours, typography or layout. The alert quoted on the page reads "Your computer was restarted because of a problem." — [Apple Support, 102382](https://support.apple.com/en-us/102382)
- Older macOS stopped responding after a panic, with garbled or detailed text on screen in 10.0 and 10.1, per Wikipedia. [Wikipedia, Kernel panic](https://en.wikipedia.org/wiki/Kernel_panic)
- Debug information is saved to NVRAM and written to a log on reboot (Wikipedia, 10.x). [Wikipedia, Kernel panic](https://en.wikipedia.org/wiki/Kernel_panic)

### Inferences
- Apple's design lesson for a failure screen: put the technical detail (panic log, addresses) in a log that is written to disk and offered later, not on the screen. The screen shows one plain sentence.
- Apple's modern approach splits the failure into two steps: a short restart message, then an optional report after login. That fits a hobby OS that must keep the on-screen message simple.
- The old multilingual grey-curtain approach shows that Apple once put a full-screen message over a dimmed display. A simple hobby version: dim the frame, one centred message, no hex.

### Gaps
- No Apple primary source gives the exact colours of any panic or restart screen. The colours above come from secondary descriptions and disagree on 10.2.
- No Apple source gives the exact pixel layout or font of the modern dialog.
- The screenshot-based descriptions on Wikipedia and Eclectic Light are secondary. Verify against real captures before copying specific values.

---

## Key Question 3: Typography (SF Pro, metrics, open alternatives, licensing)

### Takeaway
Apple's HIG gives sizes and weights for SF Pro in tables. macOS default body is 13 pt and the minimum is 10 pt. Apple advises against light weights for small text. The SF font licence is restrictive: the only licence text shown on Apple's fonts page limits use to UI mock-ups on Apple operating systems and forbids embedding. Inter is a suitable open alternative under the SIL OFL, with a reserved font name.

### Cited Findings
- Minimum and default sizes (Apple, cross-platform table): macOS default 13 pt, minimum 10 pt. iOS 17 pt default, 11 pt minimum. [Apple HIG, Typography](https://developer.apple.com/design/human-interface-guidelines/typography)
- Apple: "In general, avoid light font weights... prefer Regular, Medium, Semibold, or Bold font weights, and avoid Ultralight, Thin, and Light font weights, which can be difficult to see, especially when text is small." [Apple HIG, Typography](https://developer.apple.com/design/human-interface-guidelines/typography)
- Apple: "If you use a custom font with a thin weight, aim for larger than the recommended sizes to increase legibility." [Apple HIG, Typography](https://developer.apple.com/design/human-interface-guidelines/typography)
- macOS built-in text styles (Apple table, pt size / line height):

| Style | Weight | Size (pt) | Line height (pt) |
|---|---|---|---|
| Large Title | Regular | 26 | 32 |
| Title 1 | Regular | 22 | 26 |
| Title 2 | Regular | 17 | 22 |
| Title 3 | Regular | 15 | 20 |
| Headline | Bold | 13 | 16 |
| Body | Regular | 13 | 16 |
| Callout | Regular | 12 | 15 |
| Subheadline | Regular | 11 | 14 |
| Footnote | Regular | 10 | 13 |
| Caption 1 | Regular | 10 | 13 |
| Caption 2 | Medium | 10 | 13 |

  Source: [Apple HIG, Typography, macOS built-in text styles](https://developer.apple.com/design/human-interface-guidelines/typography). Tracking values for macOS are listed on the same page.
- SF is a sans-serif family including SF Pro, SF Compact, SF Mono and others. New York is a serif family. macOS does not support Dynamic Type. [Apple HIG, Typography](https://developer.apple.com/design/human-interface-guidelines/typography)
- Apple: "Follow the recommended default and minimum text sizes for each platform." [Apple HIG, Typography](https://developer.apple.com/design/human-interface-guidelines/typography)
- SF licence, on Apple's developer fonts page: "THE APPLE SAN FRANCISCO FONT IS TO BE USED SOLELY FOR CREATING MOCK-UPS OF USER INTERFACES TO BE USED IN SOFTWARE PRODUCTS RUNNING ON APPLE'S iOS, OS X OR tvOS OPERATING SYSTEMS." Section 2B: "You may not embed the Apple Font in any software programs or other products." Section 2A: use "only if you are a registered Apple Developer." [Apple Developer, Fonts](https://developer.apple.com/fonts/)
- The Apple fonts page offers an SF Pro download, but the only licence text shown is for SF and SF Compact. The page does not say which licence governs the SF Pro download. [Apple Developer, Fonts](https://developer.apple.com/fonts/)
- Inter: "Inter is licensed under the SIL Open Font License." "Inter" is a Reserved Font Name, so modified versions cannot use the name. Derivatives "cannot be released under any other type of license." [Inter, GitHub](https://github.com/rsms/inter)
  - Conflict: the same README also says "you are NOT allowed to sell Inter itself or derivatives of Inter." This goes beyond the standard OFL 1.1 wording. Read LICENSE.txt before relying on either statement.
- Inter's latest release is listed as 4.1 (16 November 2024), per Wikipedia's infobox (secondary). [Wikipedia, Inter (typeface)](https://en.wikipedia.org/wiki/Inter_(typeface))

### Inferences
- Do not ship SF Pro in the hobby OS. The licence forbids embedding, and the only licence text shown limits use to Apple-OS mock-ups. Use it only as a visual reference for proportions in a mock-up if the licence allows that.
- Inter is the practical choice to rasterise. Its OFL licence permits derivatives, but the reserved name means a bitmap font derived from it must not be called "Inter". Name it something else (for example "HobbyOS Sans").
- Use the macOS table as a size reference: a 13 pt body and 22 pt title scale. For a bitmap font, pick integer pixel sizes near these and avoid Light weights.

### Gaps
- Exact SF Pro tracking values for each size are only in a third-party listing, not used here.
- Whether the SF Pro download licence is the same as the SF font licence shown is unresolved.
- Inter's commercial-use and sale language is ambiguous between the README and the OFL. Check LICENSE.txt.

---

## Key Question 4: Colour, contrast and Apple's dark-mode grey scale

### Takeaway
Apple's HIG sets a 4.5:1 minimum for foreground and background contrast and recommends 7:1 for custom colours, especially small text. WCAG 2.2 sets 4.5:1 for normal text and 3:1 for large text (18 pt, or 14 pt bold). Apple's dark-mode grey scale is published as RGB in the iOS/iPadOS table. Converting those RGB values gives the hex codes quoted below.

### Cited Findings
- Apple HIG, Dark Mode: "At a minimum, make sure the contrast ratio between colors is no lower than 4.5:1. For custom foreground and background colors, strive for a contrast ratio of 7:1, especially in small text." [Apple HIG, Dark Mode](https://developer.apple.com/design/human-interface-guidelines/dark-mode)
- Apple HIG, Dark Mode: "The color palette in Dark Mode includes dimmer background colors and brighter foreground colors." Base and elevated background sets are used to show depth. [Apple HIG, Dark Mode](https://developer.apple.com/design/human-interface-guidelines/dark-mode)
- Apple HIG, Dark Mode: "Soften the color of white backgrounds." [Apple HIG, Dark Mode](https://developer.apple.com/design/human-interface-guidelines/dark-mode)
- Apple HIG, Color: "Avoid relying solely on color to differentiate between objects, indicate interactivity, or communicate essential information." [Apple HIG, Color](https://developer.apple.com/design/human-interface-guidelines/color)
- Apple HIG, Color, system colours: "Avoid hard-coding system color values in your app. Documented color values are for your reference during the app design process. The actual color values may fluctuate from release to release." [Apple HIG, Color](https://developer.apple.com/design/human-interface-guidelines/color)
- Apple HIG, Color, iOS/iPadOS system grey table, dark default (RGB as published, hex converted here):

| Name | RGB (dark, default) | Hex (converted) |
|---|---|---|
| systemGray6 | 28, 28, 30 | #1C1C1E |
| systemGray5 | 44, 44, 46 | #2C2C2E |
| systemGray4 | 58, 58, 60 | #3A3A3C |
| systemGray3 | 72, 72, 74 | #48484A |
| systemGray2 | 99, 99, 102 | #636366 |
| systemGray | 142, 142, 147 | #8E8E93 |

  Source: [Apple HIG, Color, Specifications](https://developer.apple.com/design/human-interface-guidelines/color). These are iOS/iPadOS values, not macOS. The hex column is a conversion from the published RGB and is not quoted from Apple.
- Apple's default dark system red (iOS table, dark): RGB 255, 66, 69 (#FF4245). Light: 255, 56, 60 (#FF383C). Source: [Apple HIG, Color, Specifications](https://developer.apple.com/design/human-interface-guidelines/color). Apple publishes these as reference only.
- WCAG 2.2, SC 1.4.3: "The visual presentation of text and images of text has a contrast ratio of at least 4.5:1." Large-scale text is "at least 18 point or 14 point bold," with 3:1 minimum. "Text that is part of a logo or brand name has no contrast requirement." [W3C, Understanding SC 1.4.3](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum)
- Apple HIG on hard black: none found. The HIG does not specify #000000 for dark mode.

### Inferences
- Use the grey scale above as the palette for a dark boot and failure screen. The black base, a 1C1C1E panel and 8E8E93 secondary text fit Apple's dark-mode grey system. Check contrast of 8E8E93 on 1C1C1E or 000000 with a script before use. Not computed here.
- Saturated red is not a sourced Apple choice for failure screens. Apple's own red is a system accent. A plain-red panic screen would be a design choice, not something Apple's sources endorse. Apple's sources warn against using colour alone to signal state, so pair any red with text.
- Keep body text at Apple's near-white primary label and use the grey scale for secondary text. Use a 4.5:1 minimum for all text.

### Gaps
- No Apple source gives a specific hex value for pure black, the boot-screen background, or the macOS label colours (labelColor etc.) in dark mode. The grey table above is iOS/iPadOS.
- Apple's macOS colour table is in the HIG but its numeric values were not retrieved.
- Apple's dark-mode label RGB values were not retrieved. Only the grey scale and red were.

---

## Key Question 5: Selection highlight, spacing and corner radii

### Takeaway
Apple's HIG gives no numeric spec for selection-pill radius or padding that could be fetched. Apple's guidance for selection is to use system colours and avoid relying on colour alone. The concentric-radius rule comes from visionOS guidance and is not iOS or macOS specific.

### Cited Findings
- Apple HIG, Color: "Avoid relying solely on color to differentiate between objects, indicate interactivity, or communicate essential information... you can use text labels or glyph shapes to identify objects or states." [Apple HIG, Color](https://developer.apple.com/design/human-interface-guidelines/color)
- Apple HIG, Color (macOS): the system selection colours are named, for example "Selected content background color," "Selected text background color" and "Keyboard focus indicator color" (the focus ring). No numeric values or radii are given on the page. [Apple HIG, Color](https://developer.apple.com/design/human-interface-guidelines/color)
- Apple HIG, Color: "Avoid hard-coding system color values in your app." [Apple HIG, Color](https://developer.apple.com/design/human-interface-guidelines/color)
- Concentric radius rule (visionOS guidance, not macOS): "Always ensure nested elements have relative corner radii and padding that keeps them concentric with each other." This is a third-party article quoting visionOS guidance and was not checked against Apple's page. [UX Planet, Apple Vision Pro Spatial Design 101 (Part 4)](https://uxplanet.org/apple-vision-pro-spatial-design-101-part-4-1efa5abc233a) (secondary, unverified)
- Apple's HIG dark-mode page: "Prefer the system background colors." [Apple HIG, Dark Mode](https://developer.apple.com/design/human-interface-guidelines/dark-mode)

### Inferences
- A rounded pill selection on a dark panel is consistent with Apple's visual language, but Apple's text does not specify the pill radius or padding. Reconstruct it from the concentric rule: outer radius = inner radius + padding.
- Use a neutral grey highlight (for example 2C2C2E or 3A3A3C from Apple's grey scale) rather than a saturated accent, so the selection does not read as an alert. This is an inference from Apple's use of grey surfaces, not an Apple instruction.
- Keep the non-colour cue: a text label, a bracket or a shape change, so the selected item is visible without colour.

### Gaps
- Apple's exact selection-pill radius, padding and accent-blue value were not found in fetched primary sources. The 8pt grid and capsule-radius rules found in search are third-party and unverified, and are not used here.
- Whether iOS 26 list selection corners can be changed is unresolved in the sources found.

---

## Key Question 6: Wording guidance for error screens

### Takeaway
Apple's HIG Writing page asks for plain, short, blame-free error text that says what to do next. It advises against "oops" style interjections and against "we" in error messages. The current macOS restart message fits this: one plain sentence, no technical data, and an action in a separate step.

### Cited Findings
- Apple HIG, Writing: "When an error message is necessary, display it as close to the problem as possible, avoid blame, and be clear about what someone can do to fix it." [Apple HIG, Writing](https://developer.apple.com/design/human-interface-guidelines/writing)
- Apple HIG, Writing: "Interjections like 'oops!' or 'uh-oh' are typically unnecessary and can sound insincere." [Apple HIG, Writing](https://developer.apple.com/design/human-interface-guidelines/writing)
- Apple HIG, Writing: "Avoid using we altogether because it may be unclear who the 'we' in question refers to. This is particularly problematic in error messages like 'We're having trouble loading this content.' Something like 'Unable to load content' is much clearer." [Apple HIG, Writing](https://developer.apple.com/design/human-interface-guidelines/writing)
- Apple HIG, Writing: "Be clear... If you can use fewer words, do so. When in doubt, read your writing out loud." [Apple HIG, Writing](https://developer.apple.com/design/human-interface-guidelines/writing)
- Apple HIG, Writing: "Write for everyone... Choose simple, plain language... avoiding jargon." [Apple HIG, Writing](https://developer.apple.com/design/human-interface-guidelines/writing)
- Apple's current restart text: "Your computer was restarted because of a problem." The advice (update, safe mode, reconnect devices, Apple Diagnostics) sits in the support article, not on screen. [Apple Support, 102382](https://support.apple.com/en-us/102382)

### Inferences
- A hobby failure screen could follow this pattern: a plain statement of what happened, then one line of what to do (for example "Restart your computer. Hold the power button if the screen does not respond."). No hex addresses, no register dumps, no panic codes on screen. Put those in a log file.
- Use sentence case or consistent title case, and avoid "we".

### Gaps
- Apple's HIG does not give a fixed word count or character limit for error screens.

---

## Key Question 7: Apple's restraint, widely cited write-ups, and combined recommendations

### Takeaway
Sources on Apple's restraint are mostly secondary practitioner blogs. The defensible points are that Apple's product pages show one focal item with generous whitespace, and that Apple's HIG asks for dimmer dark-mode backgrounds with brighter foregrounds. No primary Apple essay on boot-screen minimalism was found.

### Cited Findings
- A SnappyMob article says Apple's pages let you focus on "one single product, one headline, and one catchy subheadline." Secondary, blog. [SnappyMob blog](https://blog.snappymob.com/?p=46)
- A KSOM blog notes that Apple's iPhone 16 page includes "only a few details: one image of the phone, a feature mention, and a pricing button." Secondary, blog. [KSOM blog](https://blog.ksom.ac.in/?p=26586)
- A 20i article makes a related point that whitespace decides where attention goes, and warns that a minimal full-width aesthetic will not suit everyone. Secondary, blog. [20i blog](https://www.20i.com/blog/?p=15853)
- Apple HIG, Dark Mode: "In Dark Mode, the system uses a dark color palette for all screens, views, menus, and controls, and may also use greater perceptual contrast to make foreground content stand out against the darker backgrounds." [Apple HIG, Dark Mode](https://developer.apple.com/design/human-interface-guidelines/dark-mode)
- Apple HIG, Typography: "Use font sizes that most people can read easily." [Apple HIG, Typography](https://developer.apple.com/design/human-interface-guidelines/typography)

### Inferences
- The "one focal element, generous space, few words" model is consistent across Apple's boot screen support description, its restart message and its product pages. It is a reasonable target for a hobby OS: one mark or message, one bar if needed, nothing else.
- Imitate the feel with structure (centred element, large margins, one sentence), not with Apple's marks or fonts.

### Combined inferences for the hobby OS
- Layout: black or near-black background (approximate, not Apple-sourced), one centred mark or sentence, thin progress bar, large margins.
- Colour: Apple's dark grey scale (1C1C1E, 2C2C2E, 3A3A3C, 48484A, 636366, 8E8E93) for panels and secondary text, near-white for primary text, 4.5:1 minimum for all text. Do not rely on red alone.
- Typography: an Inter-derived bitmap font under a different name (reserved-name rule). Do not ship SF Pro. Body at 13 pt-equivalent or larger; avoid light weights.
- Wording: one plain sentence for what happened, one line for what to do, no "we", no "oops", no hex or panic codes on screen. Technical detail goes to a log.
- Verify before relying on it: Inter LICENSE.txt, the SF Pro download licence, and real captures of panic-screen colours.

### Gaps
- No Apple-authored or widely cited primary design essay on boot-screen restraint was found.
- Blog sources are low-authority and were used only for the general point.
