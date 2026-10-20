//! Canonical upstream colour palettes and their semantic role mappings.
//!
//! The [`Palette`] values are copied from each project's published palette,
//! never generated or interpolated. [`Roles`] only names colours that exist in
//! the canonical palette; a role upstream does not define is mapped to its
//! closest published colour and noted inline.
//!
//! Sources:
//! - Catppuccin: <https://github.com/catppuccin/palette/blob/main/palette.json>
//! - Nord: <https://www.nordtheme.com/docs/colors-and-palettes>
//! - Dracula: <https://github.com/dracula/dracula-theme#color-palette>
//! - Gruvbox: <https://github.com/morhetz/gruvbox/blob/master/colors/gruvbox.vim>
//! - Tokyo Night: <https://github.com/folke/tokyonight.nvim/blob/main/lua/tokyonight/colors/night.lua>
//! - Solarized: <https://github.com/altercation/solarized#the-values>
//! - Rosé Pine: <https://github.com/rose-pine/palette/blob/main/palette.json>
//! - One Dark: <https://github.com/joshdick/onedark.vim#color-reference>
//! - Kanagawa: <https://github.com/rebelot/kanagawa.nvim/blob/master/lua/kanagawa/colors.lua>
//! - Everforest: <https://github.com/sainnhe/everforest/blob/master/autoload/everforest.vim>
//! - Ayu: <https://github.com/ayu-theme/ayu-colors/blob/master/themes/dark.yaml>

pub type Rgb = (u8, u8, u8);

/// `0xRRGGBB` as an RGB triple.
const fn rgb(hex: u32) -> Rgb {
    ((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The published upstream palette, kept separate from semantic roles.
pub struct Palette {
    /// CLI name, e.g. `tokyo-night`.
    pub name: &'static str,
    /// The exact upstream variant, e.g. `Tokyo Night (night)`.
    pub variant: &'static str,
    /// Upstream colour names and their exact RGB values.
    pub colors: &'static [(&'static str, Rgb)],
}

impl Palette {
    pub fn rgb(&self, name: &str) -> Option<Rgb> {
        self.colors
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&(_, c)| c)
    }
}

/// Semantic roles, each naming an entry in the canonical palette.
pub struct Roles {
    pub bg: &'static str,
    pub surface: &'static str,
    pub overlay: &'static str,
    pub muted: &'static str,
    pub text: &'static str,
    pub subtext: &'static str,
    pub accent: &'static str,
    pub red: &'static str,
    pub orange: &'static str,
    pub yellow: &'static str,
    pub green: &'static str,
    pub teal: &'static str,
    pub cyan: &'static str,
    pub blue: &'static str,
    pub purple: &'static str,
    pub pink: &'static str,
    pub border: &'static str,
}

impl Roles {
    /// Preview order used by `--list-schemes` and the role test.
    pub fn all(&self) -> [(&'static str, &'static str); 17] {
        [
            ("bg", self.bg),
            ("surface", self.surface),
            ("overlay", self.overlay),
            ("muted", self.muted),
            ("fg", self.text),
            ("subtext", self.subtext),
            ("accent", self.accent),
            ("red", self.red),
            ("orange", self.orange),
            ("yellow", self.yellow),
            ("green", self.green),
            ("teal", self.teal),
            ("cyan", self.cyan),
            ("blue", self.blue),
            ("purple", self.purple),
            ("pink", self.pink),
            ("border", self.border),
        ]
    }
}

pub struct Scheme {
    pub palette: &'static Palette,
    pub aliases: &'static [&'static str],
    pub roles: Roles,
}

impl Scheme {
    /// Canonical RGB for a role name. Panics only if the static data is wrong;
    /// `semantic_roles_reference_canonical_colours` guards that in tests.
    pub fn rgb(&self, role: &str) -> Rgb {
        self.palette
            .rgb(role)
            .unwrap_or_else(|| panic!("{}: missing canonical colour '{role}'", self.palette.name))
    }
}

// ── Catppuccin ────────────────────────────────────────────

/// Catppuccin Mocha — <https://github.com/catppuccin/palette/blob/main/palette.json>
static MOCHA: Scheme = Scheme {
    palette: &Palette {
        name: "mocha",
        variant: "Catppuccin Mocha",
        colors: &[
            ("rosewater", rgb(0xf5e0dc)),
            ("flamingo", rgb(0xf2cdcd)),
            ("pink", rgb(0xf5c2e7)),
            ("mauve", rgb(0xcba6f7)),
            ("red", rgb(0xf38ba8)),
            ("maroon", rgb(0xeba0ac)),
            ("peach", rgb(0xfab387)),
            ("yellow", rgb(0xf9e2af)),
            ("green", rgb(0xa6e3a1)),
            ("teal", rgb(0x94e2d5)),
            ("sky", rgb(0x89dceb)),
            ("sapphire", rgb(0x74c7ec)),
            ("blue", rgb(0x89b4fa)),
            ("lavender", rgb(0xb4befe)),
            ("text", rgb(0xcdd6f4)),
            ("subtext1", rgb(0xbac2de)),
            ("subtext0", rgb(0xa6adc8)),
            ("overlay2", rgb(0x9399b2)),
            ("overlay1", rgb(0x7f849c)),
            ("overlay0", rgb(0x6c7086)),
            ("surface2", rgb(0x585b70)),
            ("surface1", rgb(0x45475a)),
            ("surface0", rgb(0x313244)),
            ("base", rgb(0x1e1e2e)),
            ("mantle", rgb(0x181825)),
            ("crust", rgb(0x11111b)),
        ],
    },
    aliases: &["catppuccin", "catppuccin-mocha"],
    roles: Roles {
        bg: "base",
        surface: "surface0",
        overlay: "overlay0",
        muted: "overlay1",
        text: "text",
        subtext: "subtext1",
        accent: "blue",
        red: "red",
        orange: "peach",
        yellow: "yellow",
        green: "green",
        teal: "teal",
        cyan: "sky",
        blue: "blue",
        purple: "mauve",
        pink: "pink",
        border: "surface1",
    },
};

/// Catppuccin Latte — <https://github.com/catppuccin/palette/blob/main/palette.json>
static LATTE: Scheme = Scheme {
    palette: &Palette {
        name: "latte",
        variant: "Catppuccin Latte",
        colors: &[
            ("rosewater", rgb(0xdc8a78)),
            ("flamingo", rgb(0xdd7878)),
            ("pink", rgb(0xea76cb)),
            ("mauve", rgb(0x8839ef)),
            ("red", rgb(0xd20f39)),
            ("maroon", rgb(0xe64553)),
            ("peach", rgb(0xfe640b)),
            ("yellow", rgb(0xdf8e1d)),
            ("green", rgb(0x40a02b)),
            ("teal", rgb(0x179299)),
            ("sky", rgb(0x04a5e5)),
            ("sapphire", rgb(0x209fb5)),
            ("blue", rgb(0x1e66f5)),
            ("lavender", rgb(0x7287fd)),
            ("text", rgb(0x4c4f69)),
            ("subtext1", rgb(0x5c5f77)),
            ("subtext0", rgb(0x6c6f85)),
            ("overlay2", rgb(0x7c7f93)),
            ("overlay1", rgb(0x8c8fa1)),
            ("overlay0", rgb(0x9ca0b0)),
            ("surface2", rgb(0xacb0be)),
            ("surface1", rgb(0xbcc0cc)),
            ("surface0", rgb(0xccd0da)),
            ("base", rgb(0xeff1f5)),
            ("mantle", rgb(0xe6e9ef)),
            ("crust", rgb(0xdce0e8)),
        ],
    },
    aliases: &["catppuccin-latte"],
    roles: Roles {
        bg: "base",
        surface: "surface0",
        overlay: "overlay0",
        muted: "overlay1",
        text: "text",
        subtext: "subtext1",
        accent: "blue",
        red: "red",
        orange: "peach",
        yellow: "yellow",
        green: "green",
        teal: "teal",
        cyan: "sky",
        blue: "blue",
        purple: "mauve",
        pink: "pink",
        border: "surface1",
    },
};

/// Catppuccin Frappé — <https://github.com/catppuccin/palette/blob/main/palette.json>
static FRAPPE: Scheme = Scheme {
    palette: &Palette {
        name: "frappe",
        variant: "Catppuccin Frappé",
        colors: &[
            ("rosewater", rgb(0xf2d5cf)),
            ("flamingo", rgb(0xeebebe)),
            ("pink", rgb(0xf4b8e4)),
            ("mauve", rgb(0xca9ee6)),
            ("red", rgb(0xe78284)),
            ("maroon", rgb(0xea999c)),
            ("peach", rgb(0xef9f76)),
            ("yellow", rgb(0xe5c890)),
            ("green", rgb(0xa6d189)),
            ("teal", rgb(0x81c8be)),
            ("sky", rgb(0x99d1db)),
            ("sapphire", rgb(0x85c1dc)),
            ("blue", rgb(0x8caaee)),
            ("lavender", rgb(0xbabbf1)),
            ("text", rgb(0xc6d0f5)),
            ("subtext1", rgb(0xb5bfe2)),
            ("subtext0", rgb(0xa5adce)),
            ("overlay2", rgb(0x949cbb)),
            ("overlay1", rgb(0x838ba7)),
            ("overlay0", rgb(0x737994)),
            ("surface2", rgb(0x626880)),
            ("surface1", rgb(0x51576d)),
            ("surface0", rgb(0x414559)),
            ("base", rgb(0x303446)),
            ("mantle", rgb(0x292c3c)),
            ("crust", rgb(0x232634)),
        ],
    },
    aliases: &["frappé", "catppuccin-frappe", "catppuccin-frappé"],
    roles: Roles {
        bg: "base",
        surface: "surface0",
        overlay: "overlay0",
        muted: "overlay1",
        text: "text",
        subtext: "subtext1",
        accent: "blue",
        red: "red",
        orange: "peach",
        yellow: "yellow",
        green: "green",
        teal: "teal",
        cyan: "sky",
        blue: "blue",
        purple: "mauve",
        pink: "pink",
        border: "surface1",
    },
};

/// Catppuccin Macchiato — <https://github.com/catppuccin/palette/blob/main/palette.json>
static MACCHIATO: Scheme = Scheme {
    palette: &Palette {
        name: "macchiato",
        variant: "Catppuccin Macchiato",
        colors: &[
            ("rosewater", rgb(0xf4dbd6)),
            ("flamingo", rgb(0xf0c6c6)),
            ("pink", rgb(0xf5bde6)),
            ("mauve", rgb(0xc6a0f6)),
            ("red", rgb(0xed8796)),
            ("maroon", rgb(0xee99a0)),
            ("peach", rgb(0xf5a97f)),
            ("yellow", rgb(0xeed49f)),
            ("green", rgb(0xa6da95)),
            ("teal", rgb(0x8bd5ca)),
            ("sky", rgb(0x91d7e3)),
            ("sapphire", rgb(0x7dc4e4)),
            ("blue", rgb(0x8aadf4)),
            ("lavender", rgb(0xb7bdf8)),
            ("text", rgb(0xcad3f5)),
            ("subtext1", rgb(0xb8c0e0)),
            ("subtext0", rgb(0xa5adcb)),
            ("overlay2", rgb(0x939ab7)),
            ("overlay1", rgb(0x8087a2)),
            ("overlay0", rgb(0x6e738d)),
            ("surface2", rgb(0x5b6078)),
            ("surface1", rgb(0x494d64)),
            ("surface0", rgb(0x363a4f)),
            ("base", rgb(0x24273a)),
            ("mantle", rgb(0x1e2030)),
            ("crust", rgb(0x181926)),
        ],
    },
    aliases: &["catppuccin-macchiato"],
    roles: Roles {
        bg: "base",
        surface: "surface0",
        overlay: "overlay0",
        muted: "overlay1",
        text: "text",
        subtext: "subtext1",
        accent: "blue",
        red: "red",
        orange: "peach",
        yellow: "yellow",
        green: "green",
        teal: "teal",
        cyan: "sky",
        blue: "blue",
        purple: "mauve",
        pink: "pink",
        border: "surface1",
    },
};

// ── Nord ──────────────────────────────────────────────────

/// Nord — <https://www.nordtheme.com/docs/colors-and-palettes>
static NORD: Scheme = Scheme {
    palette: &Palette {
        name: "nord",
        variant: "Nord",
        colors: &[
            ("nord0", rgb(0x2e3440)),
            ("nord1", rgb(0x3b4252)),
            ("nord2", rgb(0x434c5e)),
            ("nord3", rgb(0x4c566a)),
            ("nord4", rgb(0xd8dee9)),
            ("nord5", rgb(0xe5e9f0)),
            ("nord6", rgb(0xeceff4)),
            ("nord7", rgb(0x8fbcbb)),
            ("nord8", rgb(0x88c0d0)),
            ("nord9", rgb(0x81a1c1)),
            ("nord10", rgb(0x5e81ac)),
            ("nord11", rgb(0xbf616a)),
            ("nord12", rgb(0xd08770)),
            ("nord13", rgb(0xebcb8b)),
            ("nord14", rgb(0xa3be8c)),
            ("nord15", rgb(0xb48ead)),
        ],
    },
    aliases: &[],
    roles: Roles {
        bg: "nord0",
        surface: "nord1",
        overlay: "nord3",
        muted: "nord3",
        text: "nord6",
        subtext: "nord4",
        accent: "nord8",
        red: "nord11",
        orange: "nord12",
        yellow: "nord13",
        green: "nord14",
        teal: "nord7",
        cyan: "nord8",
        blue: "nord10",
        purple: "nord15",
        // Nord has no pink; nord15 is its only violet.
        pink: "nord15",
        border: "nord2",
    },
};

// ── Dracula ───────────────────────────────────────────────

/// Dracula (dark) — <https://github.com/dracula/dracula-theme#color-palette>
static DRACULA: Scheme = Scheme {
    palette: &Palette {
        name: "dracula",
        variant: "Dracula (dark)",
        colors: &[
            ("background", rgb(0x282a36)),
            ("currentLine", rgb(0x44475a)),
            ("selection", rgb(0x44475a)),
            ("foreground", rgb(0xf8f8f2)),
            ("comment", rgb(0x6272a4)),
            ("cyan", rgb(0x8be9fd)),
            ("green", rgb(0x50fa7b)),
            ("orange", rgb(0xffb86c)),
            ("pink", rgb(0xff79c6)),
            ("purple", rgb(0xbd93f9)),
            ("red", rgb(0xff5555)),
            ("yellow", rgb(0xf1fa8c)),
        ],
    },
    aliases: &[],
    roles: Roles {
        bg: "background",
        surface: "currentLine",
        overlay: "comment",
        muted: "comment",
        text: "foreground",
        subtext: "foreground",
        accent: "purple",
        red: "red",
        orange: "orange",
        yellow: "yellow",
        green: "green",
        teal: "cyan",
        cyan: "cyan",
        // Dracula has no blue; its violet is the canonical substitute.
        blue: "purple",
        purple: "purple",
        pink: "pink",
        border: "currentLine",
    },
};

// ── Gruvbox ───────────────────────────────────────────────

/// Gruvbox Dark (medium) — <https://github.com/morhetz/gruvbox/blob/master/colors/gruvbox.vim>
static GRUVBOX: Scheme = Scheme {
    palette: &Palette {
        name: "gruvbox",
        variant: "Gruvbox Dark (medium)",
        colors: &[
            ("dark0_hard", rgb(0x1d2021)),
            ("dark0", rgb(0x282828)),
            ("dark0_soft", rgb(0x32302f)),
            ("dark1", rgb(0x3c3836)),
            ("dark2", rgb(0x504945)),
            ("dark3", rgb(0x665c54)),
            ("dark4", rgb(0x7c6f64)),
            ("gray", rgb(0x928374)),
            ("light0_hard", rgb(0xf9f5d7)),
            ("light0", rgb(0xfbf1c7)),
            ("light0_soft", rgb(0xf2e5bc)),
            ("light1", rgb(0xebdbb2)),
            ("light2", rgb(0xd5c4a1)),
            ("light3", rgb(0xbdae93)),
            ("light4", rgb(0xa89984)),
            ("bright_red", rgb(0xfb4934)),
            ("bright_green", rgb(0xb8bb26)),
            ("bright_yellow", rgb(0xfabd2f)),
            ("bright_blue", rgb(0x83a598)),
            ("bright_purple", rgb(0xd3869b)),
            ("bright_aqua", rgb(0x8ec07c)),
            ("bright_orange", rgb(0xfe8019)),
            ("neutral_red", rgb(0xcc241d)),
            ("neutral_green", rgb(0x98971a)),
            ("neutral_yellow", rgb(0xd79921)),
            ("neutral_blue", rgb(0x458588)),
            ("neutral_purple", rgb(0xb16286)),
            ("neutral_aqua", rgb(0x689d6a)),
            ("neutral_orange", rgb(0xd65d0e)),
        ],
    },
    aliases: &["gruvbox-dark"],
    roles: Roles {
        bg: "dark0",
        surface: "dark1",
        overlay: "dark2",
        muted: "gray",
        text: "light1",
        subtext: "light2",
        accent: "bright_yellow",
        red: "bright_red",
        orange: "bright_orange",
        yellow: "bright_yellow",
        green: "bright_green",
        teal: "bright_aqua",
        // Gruvbox has no cyan; the bright blue is its closest canonical cool hue.
        cyan: "bright_blue",
        blue: "bright_blue",
        purple: "bright_purple",
        // No pink; bright purple stands in.
        pink: "bright_purple",
        border: "dark2",
    },
};

// ── Tokyo Night ───────────────────────────────────────────

/// Tokyo Night (night) — storm palette with the night background overrides.
/// <https://github.com/folke/tokyonight.nvim/blob/main/lua/tokyonight/colors/night.lua>
static TOKYO_NIGHT: Scheme = Scheme {
    palette: &Palette {
        name: "tokyo-night",
        variant: "Tokyo Night (night)",
        colors: &[
            ("bg", rgb(0x1a1b26)),
            ("bg_dark", rgb(0x16161e)),
            ("bg_dark1", rgb(0x0c0e14)),
            ("bg_highlight", rgb(0x292e42)),
            ("blue", rgb(0x7aa2f7)),
            ("blue0", rgb(0x3d59a1)),
            ("blue1", rgb(0x2ac3de)),
            ("blue2", rgb(0x0db9d7)),
            ("blue5", rgb(0x89ddff)),
            ("blue6", rgb(0xb4f9f8)),
            ("blue7", rgb(0x394b70)),
            ("comment", rgb(0x565f89)),
            ("cyan", rgb(0x7dcfff)),
            ("dark3", rgb(0x545c7e)),
            ("dark5", rgb(0x737aa2)),
            ("fg", rgb(0xc0caf5)),
            ("fg_dark", rgb(0xa9b1d6)),
            ("fg_gutter", rgb(0x3b4261)),
            ("green", rgb(0x9ece6a)),
            ("green1", rgb(0x73daca)),
            ("green2", rgb(0x41a6b5)),
            ("magenta", rgb(0xbb9af7)),
            ("magenta2", rgb(0xff007c)),
            ("orange", rgb(0xff9e64)),
            ("purple", rgb(0x9d7cd8)),
            ("red", rgb(0xf7768e)),
            ("red1", rgb(0xdb4b4b)),
            ("teal", rgb(0x1abc9c)),
            ("terminal_black", rgb(0x414868)),
            ("yellow", rgb(0xe0af68)),
        ],
    },
    aliases: &["tokyo", "tokyonight"],
    roles: Roles {
        bg: "bg",
        surface: "bg_highlight",
        overlay: "comment",
        muted: "dark3",
        text: "fg",
        subtext: "fg_dark",
        accent: "blue",
        red: "red",
        orange: "orange",
        yellow: "yellow",
        green: "green",
        teal: "teal",
        cyan: "cyan",
        blue: "blue",
        purple: "magenta",
        pink: "magenta2",
        border: "fg_gutter",
    },
};

// ── Solarized ─────────────────────────────────────────────

/// Solarized Dark — <https://github.com/altercation/solarized#the-values>
static SOLARIZED_DARK: Scheme = Scheme {
    palette: &Palette {
        name: "solarized-dark",
        variant: "Solarized Dark",
        colors: &[
            ("base03", rgb(0x002b36)),
            ("base02", rgb(0x073642)),
            ("base01", rgb(0x586e75)),
            ("base00", rgb(0x657b83)),
            ("base0", rgb(0x839496)),
            ("base1", rgb(0x93a1a1)),
            ("base2", rgb(0xeee8d5)),
            ("base3", rgb(0xfdf6e3)),
            ("yellow", rgb(0xb58900)),
            ("orange", rgb(0xcb4b16)),
            ("red", rgb(0xdc322f)),
            ("magenta", rgb(0xd33682)),
            ("violet", rgb(0x6c71c4)),
            ("blue", rgb(0x268bd2)),
            ("cyan", rgb(0x2aa198)),
            ("green", rgb(0x859900)),
        ],
    },
    aliases: &["solarized"],
    roles: Roles {
        bg: "base03",
        surface: "base02",
        overlay: "base01",
        muted: "base01",
        text: "base0",
        subtext: "base1",
        accent: "blue",
        red: "red",
        orange: "orange",
        yellow: "yellow",
        green: "green",
        teal: "cyan",
        cyan: "cyan",
        blue: "blue",
        purple: "violet",
        pink: "magenta",
        border: "base02",
    },
};

// ── Rosé Pine ─────────────────────────────────────────────

/// Rosé Pine (main) — <https://github.com/rose-pine/palette/blob/main/palette.json>
static ROSE_PINE: Scheme = Scheme {
    palette: &Palette {
        name: "rose-pine",
        variant: "Rosé Pine (main)",
        colors: &[
            ("base", rgb(0x191724)),
            ("surface", rgb(0x1f1d2e)),
            ("overlay", rgb(0x26233a)),
            ("muted", rgb(0x6e6a86)),
            ("subtle", rgb(0x908caa)),
            ("text", rgb(0xe0def4)),
            ("love", rgb(0xeb6f92)),
            ("gold", rgb(0xf6c177)),
            ("rose", rgb(0xebbcba)),
            ("pine", rgb(0x31748f)),
            ("foam", rgb(0x9ccfd8)),
            ("iris", rgb(0xc4a7e7)),
        ],
    },
    aliases: &["rosepine", "rose_pine"],
    roles: Roles {
        bg: "base",
        surface: "surface",
        overlay: "overlay",
        muted: "muted",
        text: "text",
        subtext: "subtle",
        accent: "iris",
        red: "love",
        // Rosé Pine has no orange; gold is its warm accent.
        orange: "gold",
        yellow: "gold",
        green: "pine",
        teal: "foam",
        cyan: "rose",
        blue: "foam",
        purple: "iris",
        pink: "love",
        border: "overlay",
    },
};

// ── One Dark ──────────────────────────────────────────────

/// One Dark / onedark.vim — <https://github.com/joshdick/onedark.vim#color-reference>
static ONEDARK: Scheme = Scheme {
    palette: &Palette {
        name: "onedark",
        variant: "One Dark (onedark.vim)",
        colors: &[
            ("black", rgb(0x282c34)),
            ("dark", rgb(0x21252b)),
            ("white", rgb(0xabb2bf)),
            ("light_red", rgb(0xe06c75)),
            ("dark_red", rgb(0xbe5046)),
            ("green", rgb(0x98c379)),
            ("light_yellow", rgb(0xe5c07b)),
            ("dark_yellow", rgb(0xd19a66)),
            ("blue", rgb(0x61afef)),
            ("magenta", rgb(0xc678dd)),
            ("cyan", rgb(0x56b6c2)),
            ("gutter_grey", rgb(0x4b5263)),
            ("comment_grey", rgb(0x5c6370)),
        ],
    },
    aliases: &["one-dark", "atom"],
    roles: Roles {
        bg: "black",
        surface: "dark",
        overlay: "comment_grey",
        muted: "comment_grey",
        text: "white",
        subtext: "white",
        accent: "blue",
        red: "light_red",
        // onedark.vim calls #d19a66 Dark Yellow; it is the palette's orange.
        orange: "dark_yellow",
        yellow: "light_yellow",
        green: "green",
        teal: "cyan",
        cyan: "cyan",
        blue: "blue",
        purple: "magenta",
        // No pink; magenta stands in.
        pink: "magenta",
        border: "gutter_grey",
    },
};

// ── Kanagawa ──────────────────────────────────────────────

/// Kanagawa Wave — <https://github.com/rebelot/kanagawa.nvim/blob/master/lua/kanagawa/colors.lua>
static KANAGAWA: Scheme = Scheme {
    palette: &Palette {
        name: "kanagawa",
        variant: "Kanagawa Wave",
        colors: &[
            ("sumiInk0", rgb(0x16161d)),
            ("sumiInk1", rgb(0x181820)),
            ("sumiInk2", rgb(0x1a1a22)),
            ("sumiInk3", rgb(0x1f1f28)),
            ("sumiInk4", rgb(0x2a2a37)),
            ("sumiInk5", rgb(0x363646)),
            ("sumiInk6", rgb(0x54546d)),
            ("waveBlue1", rgb(0x223249)),
            ("waveBlue2", rgb(0x2d4f67)),
            ("winterGreen", rgb(0x2b3328)),
            ("winterYellow", rgb(0x49443c)),
            ("winterRed", rgb(0x43242b)),
            ("winterBlue", rgb(0x252535)),
            ("autumnGreen", rgb(0x76946a)),
            ("autumnRed", rgb(0xc34043)),
            ("autumnYellow", rgb(0xdca561)),
            ("samuraiRed", rgb(0xe82424)),
            ("roninYellow", rgb(0xff9e3b)),
            ("waveAqua1", rgb(0x6a9589)),
            ("dragonBlue", rgb(0x658594)),
            ("oldWhite", rgb(0xc8c093)),
            ("fujiWhite", rgb(0xdcd7ba)),
            ("fujiGray", rgb(0x727169)),
            ("oniViolet", rgb(0x957fb8)),
            ("oniViolet2", rgb(0xb8b4d0)),
            ("crystalBlue", rgb(0x7e9cd8)),
            ("springViolet1", rgb(0x938aa9)),
            ("springViolet2", rgb(0x9cabca)),
            ("springBlue", rgb(0x7fb4ca)),
            ("lightBlue", rgb(0xa3d4d5)),
            ("waveAqua2", rgb(0x7aa89f)),
            ("springGreen", rgb(0x98bb6c)),
            ("boatYellow1", rgb(0x938056)),
            ("boatYellow2", rgb(0xc0a36e)),
            ("carpYellow", rgb(0xe6c384)),
            ("sakuraPink", rgb(0xd27e99)),
            ("waveRed", rgb(0xe46876)),
            ("peachRed", rgb(0xff5d62)),
            ("surimiOrange", rgb(0xffa066)),
            ("katanaGray", rgb(0x717c7c)),
        ],
    },
    aliases: &[],
    roles: Roles {
        bg: "sumiInk3",
        surface: "sumiInk4",
        overlay: "sumiInk6",
        muted: "fujiGray",
        text: "fujiWhite",
        subtext: "oldWhite",
        accent: "crystalBlue",
        red: "waveRed",
        orange: "surimiOrange",
        yellow: "carpYellow",
        green: "springGreen",
        teal: "waveAqua2",
        cyan: "springBlue",
        blue: "crystalBlue",
        purple: "oniViolet",
        pink: "sakuraPink",
        border: "sumiInk6",
    },
};

// ── Everforest ────────────────────────────────────────────

/// Everforest Dark (medium) — <https://github.com/sainnhe/everforest/blob/master/autoload/everforest.vim>
static EVERFOREST: Scheme = Scheme {
    palette: &Palette {
        name: "everforest",
        variant: "Everforest Dark (medium)",
        colors: &[
            ("bg_dim", rgb(0x232a2e)),
            ("bg0", rgb(0x2d353b)),
            ("bg1", rgb(0x343f44)),
            ("bg2", rgb(0x3d484d)),
            ("bg3", rgb(0x475258)),
            ("bg4", rgb(0x4f585e)),
            ("bg5", rgb(0x56635f)),
            ("fg", rgb(0xd3c6aa)),
            ("red", rgb(0xe67e80)),
            ("orange", rgb(0xe69875)),
            ("yellow", rgb(0xdbbc7f)),
            ("green", rgb(0xa7c080)),
            ("aqua", rgb(0x83c092)),
            ("blue", rgb(0x7fbbb3)),
            ("purple", rgb(0xd699b6)),
            ("grey0", rgb(0x7a8478)),
            ("grey1", rgb(0x859289)),
            ("grey2", rgb(0x9da9a0)),
        ],
    },
    aliases: &[],
    roles: Roles {
        bg: "bg0",
        surface: "bg1",
        overlay: "bg3",
        muted: "grey1",
        text: "fg",
        subtext: "grey2",
        accent: "green",
        red: "red",
        orange: "orange",
        yellow: "yellow",
        green: "green",
        teal: "aqua",
        // Everforest has no cyan; its blue is the only cool canonical hue.
        cyan: "blue",
        blue: "blue",
        purple: "purple",
        // No pink; purple stands in.
        pink: "purple",
        border: "bg3",
    },
};

// ── Ayu ───────────────────────────────────────────────────

/// Ayu Dark — <https://github.com/ayu-theme/ayu-colors/blob/master/themes/dark.yaml>
/// Syntax colours upstream are lightness levels derived from these base values.
static AYU: Scheme = Scheme {
    palette: &Palette {
        name: "ayu",
        variant: "Ayu Dark",
        colors: &[
            ("gray", rgb(0x5a6673)),
            ("red", rgb(0xf07178)),
            ("pink", rgb(0xf29668)),
            ("orange", rgb(0xff8f40)),
            ("peach", rgb(0xe6c08a)),
            ("yellow", rgb(0xffb454)),
            ("green", rgb(0xaad94c)),
            ("teal", rgb(0x95e6cb)),
            ("indigo", rgb(0x39bae6)),
            ("blue", rgb(0x59c2ff)),
            ("purple", rgb(0xd2a6ff)),
            ("fg", rgb(0xbfbdb6)),
            ("bg", rgb(0x10141c)),
            ("surface", rgb(0x0d1017)),
            ("line", rgb(0x161a24)),
            ("ui_line", rgb(0x1b1f29)),
            ("accent", rgb(0xe6b450)),
        ],
    },
    aliases: &["ayu-dark"],
    roles: Roles {
        bg: "bg",
        surface: "line",
        overlay: "ui_line",
        muted: "gray",
        text: "fg",
        subtext: "fg",
        accent: "accent",
        red: "red",
        orange: "orange",
        yellow: "yellow",
        green: "green",
        teal: "teal",
        cyan: "indigo",
        blue: "blue",
        purple: "purple",
        pink: "pink",
        border: "ui_line",
    },
};

/// Verified palettes, in preview order.
pub static SCHEMES: &[&Scheme] = &[
    &MOCHA,
    &LATTE,
    &FRAPPE,
    &MACCHIATO,
    &NORD,
    &DRACULA,
    &GRUVBOX,
    &TOKYO_NIGHT,
    &SOLARIZED_DARK,
    &ROSE_PINE,
    &ONEDARK,
    &KANAGAWA,
    &EVERFOREST,
    &AYU,
];

/// Case-insensitive lookup by canonical name or alias.
pub fn find(name: &str) -> Option<&'static Scheme> {
    let name = name.trim().to_lowercase();
    SCHEMES
        .iter()
        .copied()
        .find(|s| s.palette.name == name.as_str() || s.aliases.contains(&name.as_str()))
}

/// Deterministic decision rule for `--scheme auto`: light terminals get Latte,
/// dark or undetermined terminals get Mocha.
pub fn auto_for(light: bool) -> &'static Scheme {
    if light {
        &LATTE
    } else {
        &MOCHA
    }
}

/// Lower-case `#rrggbb` for display.
pub fn hex((r, g, b): Rgb) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Two-cell truecolour swatch, reset immediately so the label stays uncoloured.
pub fn swatch((r, g, b): Rgb) -> String {
    format!("\x1b[38;2;{r};{g};{b}m██\x1b[0m")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(name: &str) -> &'static Scheme {
        find(name).unwrap_or_else(|| panic!("missing scheme {name}"))
    }

    fn col(name: &str, colour: &str) -> Rgb {
        s(name)
            .palette
            .rgb(colour)
            .unwrap_or_else(|| panic!("{name} missing {colour}"))
    }

    #[test]
    fn catppuccin_palettes_match_upstream() {
        for (name, base, text, blue) in [
            ("mocha", 0x1e1e2e, 0xcdd6f4, 0x89b4fa),
            ("latte", 0xeff1f5, 0x4c4f69, 0x1e66f5),
            ("frappe", 0x303446, 0xc6d0f5, 0x8caaee),
            ("macchiato", 0x24273a, 0xcad3f5, 0x8aadf4),
        ] {
            assert_eq!(col(name, "base"), rgb(base), "{name} base");
            assert_eq!(col(name, "text"), rgb(text), "{name} text");
            assert_eq!(col(name, "blue"), rgb(blue), "{name} blue");
        }
        assert_eq!(col("mocha", "mauve"), rgb(0xcba6f7));
        assert_eq!(col("mocha", "peach"), rgb(0xfab387));
        assert_eq!(col("latte", "red"), rgb(0xd20f39));
    }

    #[test]
    fn every_scheme_has_its_upstream_anchors() {
        let anchors: &[(&str, &str, u32)] = &[
            ("nord", "nord0", 0x2e3440),
            ("nord", "nord6", 0xeceff4),
            ("nord", "nord10", 0x5e81ac),
            ("nord", "nord15", 0xb48ead),
            ("dracula", "background", 0x282a36),
            ("dracula", "foreground", 0xf8f8f2),
            ("dracula", "purple", 0xbd93f9),
            ("dracula", "red", 0xff5555),
            ("gruvbox", "dark0", 0x282828),
            ("gruvbox", "light1", 0xebdbb2),
            ("gruvbox", "bright_red", 0xfb4934),
            ("gruvbox", "bright_yellow", 0xfabd2f),
            ("tokyo-night", "bg", 0x1a1b26),
            ("tokyo-night", "fg", 0xc0caf5),
            ("tokyo-night", "blue", 0x7aa2f7),
            ("tokyo-night", "magenta2", 0xff007c),
            ("solarized-dark", "base03", 0x002b36),
            ("solarized-dark", "base0", 0x839496),
            ("solarized-dark", "blue", 0x268bd2),
            ("solarized-dark", "magenta", 0xd33682),
            ("rose-pine", "base", 0x191724),
            ("rose-pine", "text", 0xe0def4),
            ("rose-pine", "love", 0xeb6f92),
            ("rose-pine", "iris", 0xc4a7e7),
            ("onedark", "black", 0x282c34),
            ("onedark", "white", 0xabb2bf),
            ("onedark", "blue", 0x61afef),
            ("onedark", "dark_yellow", 0xd19a66),
            ("kanagawa", "sumiInk3", 0x1f1f28),
            ("kanagawa", "fujiWhite", 0xdcd7ba),
            ("kanagawa", "crystalBlue", 0x7e9cd8),
            ("kanagawa", "waveRed", 0xe46876),
            ("everforest", "bg0", 0x2d353b),
            ("everforest", "fg", 0xd3c6aa),
            ("everforest", "green", 0xa7c080),
            ("everforest", "purple", 0xd699b6),
            ("ayu", "bg", 0x10141c),
            ("ayu", "fg", 0xbfbdb6),
            ("ayu", "accent", 0xe6b450),
            ("ayu", "blue", 0x59c2ff),
        ];
        for &(name, colour, value) in anchors {
            assert_eq!(col(name, colour), rgb(value), "{name}/{colour}");
        }
    }

    #[test]
    fn lookup_and_aliases() {
        for scheme in SCHEMES.iter().copied() {
            assert!(
                find(scheme.palette.name).is_some(),
                "{}",
                scheme.palette.name
            );
        }
        for (alias, canonical) in [
            ("catppuccin", "mocha"),
            ("catppuccin-mocha", "mocha"),
            ("catppuccin-latte", "latte"),
            ("frappé", "frappe"),
            ("catppuccin-frappé", "frappe"),
            ("catppuccin-macchiato", "macchiato"),
            ("gruvbox-dark", "gruvbox"),
            ("tokyo", "tokyo-night"),
            ("tokyonight", "tokyo-night"),
            ("solarized", "solarized-dark"),
            ("rosepine", "rose-pine"),
            ("rose_pine", "rose-pine"),
            ("one-dark", "onedark"),
            ("atom", "onedark"),
            ("ayu-dark", "ayu"),
        ] {
            assert_eq!(find(alias).unwrap().palette.name, canonical, "{alias}");
        }
        assert_eq!(find("  MOCHA  ").unwrap().palette.name, "mocha");
        assert_eq!(find("FRAPPÉ").unwrap().palette.name, "frappe");
        assert!(find("no-such-scheme").is_none());
        assert!(find("").is_none());
    }

    #[test]
    fn semantic_roles_reference_canonical_colours() {
        for scheme in SCHEMES.iter().copied() {
            assert!(!scheme.palette.colors.is_empty(), "{}", scheme.palette.name);
            let roles = scheme.roles.all();
            assert_eq!(roles.len(), 17);
            for (label, role) in roles {
                assert!(
                    scheme.palette.rgb(role).is_some(),
                    "{}: {label} -> {role} is not canonical",
                    scheme.palette.name
                );
            }
        }
    }

    #[test]
    fn missing_roles_use_documented_substitutes() {
        assert_eq!(s("nord").roles.pink, "nord15");
        assert_eq!(s("dracula").roles.blue, "purple");
        assert_eq!(s("gruvbox").roles.cyan, "bright_blue");
        assert_eq!(s("gruvbox").roles.pink, "bright_purple");
        assert_eq!(s("rose-pine").roles.orange, "gold");
        assert_eq!(s("onedark").roles.orange, "dark_yellow");
        assert_eq!(s("onedark").roles.pink, "magenta");
        assert_eq!(s("everforest").roles.cyan, "blue");
        assert_eq!(s("everforest").roles.pink, "purple");
    }

    #[test]
    fn auto_is_deterministic() {
        assert_eq!(auto_for(true).palette.name, "latte");
        assert_eq!(auto_for(false).palette.name, "mocha");
    }

    #[test]
    fn swatches_are_truecolour_and_reset() {
        assert_eq!(hex((0x1e, 0x1e, 0x2e)), "#1e1e2e");
        assert_eq!(swatch((255, 0, 0)), "\x1b[38;2;255;0;0m██\x1b[0m");
    }

    #[test]
    fn names_and_aliases_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for scheme in SCHEMES.iter().copied() {
            assert!(seen.insert(scheme.palette.name), "{}", scheme.palette.name);
            for alias in scheme.aliases {
                assert!(seen.insert(*alias), "{alias}");
            }
        }
    }
}
