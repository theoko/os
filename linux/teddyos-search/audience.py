"""Detect *how* someone is asking — not a profile quiz.

Heuristic personas from the current ask only (never a permanent profile):
  code, academic, student, writer, business, design, job, teacher, data,
  founder, legal, parent, marketing, sales, finance, product, support,
  science, language, creative, health, nonprofit, policy, plus 100+ everyday
  specialist domains (fitness, trades, ML, creator economy, …), plain.

Never blocks, never forces a self-label.
"""

from __future__ import annotations

import re
from enum import Enum


class Audience(str, Enum):
    PLAIN = "plain"
    CODE = "code"  # Ultracode
    ACADEMIC = "academic"
    STUDENT = "student"
    WRITER = "writer"
    BUSINESS = "business"
    DESIGN = "design"
    JOB = "job"
    TEACHER = "teacher"
    DATA = "data"
    FOUNDER = "founder"
    LEGAL = "legal"
    PARENT = "parent"
    MARKETING = "marketing"
    SALES = "sales"
    FINANCE = "finance"
    PRODUCT = "product"
    SUPPORT = "support"
    SCIENCE = "science"
    LANGUAGE = "language"
    CREATIVE = "creative"
    HEALTH = "health"
    NONPROFIT = "nonprofit"
    POLICY = "policy"
    # Wave 3 — everyday domains
    REAL_ESTATE = "real_estate"
    TRAVEL = "travel"
    COOKING = "cooking"
    GAMING = "gaming"
    SPORTS = "sports"
    HR = "hr"
    JOURNALISM = "journalism"
    ACCESSIBILITY = "accessibility"
    ENGINEERING = "engineering"  # physical eng (not Ultracode software)
    SECURITY = "security"  # cyber / infosec
    HOSPITALITY = "hospitality"
    EVENTS = "events"
    FASHION = "fashion"
    DIY = "diy"
    ENVIRONMENT = "environment"
    SPIRITUAL = "spiritual"
    SENIOR = "senior"  # aging / elder care
    AUTOMOTIVE = "automotive"
    AGRICULTURE = "agriculture"
    MUSIC = "music"  # music craft (vs general creative)
    # Wave 4 — more everyday + specialist domains
    PHOTOGRAPHY = "photography"
    FILM = "film"
    PODCAST = "podcast"
    ARCHITECTURE = "architecture"
    INTERIOR = "interior"
    INSURANCE = "insurance"
    TAX = "tax"
    INVESTING = "investing"
    CRYPTO = "crypto"
    RETAIL = "retail"
    ECOMMERCE = "ecommerce"
    LOGISTICS = "logistics"
    MANUFACTURING = "manufacturing"
    CONSTRUCTION = "construction"
    ROBOTICS = "robotics"
    MATH = "math"
    PHILOSOPHY = "philosophy"
    PETS = "pets"
    CHILDCARE = "childcare"
    IMMIGRATION = "immigration"
    THERAPY = "therapy"  # mental health support framing (not clinical care)
    LIBRARY = "library"
    THEATER = "theater"
    DANCE = "dance"
    WEATHER = "weather"
    ASTRONOMY = "astronomy"
    COMPLIANCE = "compliance"
    OPERATIONS = "operations"
    PROCUREMENT = "procurement"
    QUALITY = "quality"
    GROWTH = "growth"  # growth eng / growth marketing hybrid
    UX_RESEARCH = "ux_research"
    STATS = "stats"
    GENEALOGY = "genealogy"
    COLLECTING = "collecting"
    OUTDOORS = "outdoors"
    GARDENING = "gardening"
    BAKING = "baking"
    COFFEE = "coffee"
    WINE = "wine"
    BEER = "beer"
    AVIATION = "aviation"
    MARITIME = "maritime"
    ENERGY = "energy"
    TELECOM = "telecom"
    MEDIA = "media"
    PR = "pr"
    SOCIAL_WORK = "social_work"
    ACCOUNTING = "accounting"
    ACTING = "acting"
    COMEDY = "comedy"
    WOODWORKING = "woodworking"
    METALWORKING = "metalworking"
    ELECTRONICS = "electronics"
    PRINTING_3D = "printing_3d"
    SEWING = "sewing"
    KNITTING = "knitting"
    CHESS = "chess"
    TABLETOP = "tabletop"
    ANIME = "anime"
    COMICS = "comics"
    SCUBA = "scuba"
    CYCLING = "cycling"
    RUNNING = "running"
    MARTIAL_ARTS = "martial_arts"
    NUTRITION = "nutrition"
    PRODUCTIVITY = "productivity"
    PKM = "pkm"
    DEVOPS = "devops"
    CLOUD = "cloud"
    NETWORKING = "networking"
    DATABASE = "database"
    MOBILE = "mobile"
    WEBDEV = "webdev"
    EMBEDDED = "embedded"
    IOT = "iot"
    ARVR = "arvr"
    FREELANCE = "freelance"
    CONSULTING = "consulting"
    COACHING = "coaching"
    SPEAKING = "speaking"
    RELATIONSHIPS = "relationships"
    DATING = "dating"
    HISTORY = "history"
    GEOGRAPHY = "geography"
    CHEMISTRY = "chemistry"
    BIOLOGY = "biology"
    PHYSICS = "physics"
    MEDICINE = "medicine"
    NURSING = "nursing"
    PHARMACY = "pharmacy"
    DENTAL = "dental"
    VETERINARY = "veterinary"
    MILITARY = "military"
    FIRE = "fire"
    POLICE = "police"
    GEOLOGY = "geology"
    OCEAN = "ocean"
    ARCHAEOLOGY = "archaeology"
    LINGUISTICS = "linguistics"
    # Wave 6 — more everyday + specialist domains
    FITNESS = "fitness"
    YOGA = "yoga"
    CLIMBING = "climbing"
    GOLF = "golf"
    FISHING = "fishing"
    SWIMMING = "swimming"
    SKIING = "skiing"
    MOTORCYCLE = "motorcycle"
    DRONE = "drone"
    GAME_DEV = "game_dev"
    ANIMATION = "animation"
    POETRY = "poetry"
    MAKEUP = "makeup"
    HAIR = "hair"
    SKINCARE = "skincare"
    WEDDING = "wedding"
    PREGNANCY = "pregnancy"
    SLEEP = "sleep"
    FIRST_AID = "first_aid"
    PUBLIC_HEALTH = "public_health"
    ML_AI = "ml_ai"
    SRE = "sre"
    SYSTEM_DESIGN = "system_design"
    TECH_WRITING = "tech_writing"
    PROJECT_MGMT = "project_mgmt"
    AGILE = "agile"
    REMOTE_WORK = "remote_work"
    CONTENT_CREATOR = "content_creator"
    SEO = "seo"
    BRAND = "brand"
    NEGOTIATION = "negotiation"
    PATENT = "patent"
    HOMESCHOOL = "homeschool"
    TEST_PREP = "test_prep"
    BARTENDING = "bartending"
    TEA = "tea"
    BBQ = "bbq"
    BEEKEEPING = "beekeeping"
    AQUARIUM = "aquarium"
    BIRDING = "birding"
    HORSES = "horses"
    SURVIVAL = "survival"
    SMART_HOME = "smart_home"
    AUDIO_HIFI = "audio_hifi"
    WATCHES = "watches"
    JEWELRY = "jewelry"
    CERAMICS = "ceramics"
    CALLIGRAPHY = "calligraphy"
    LEGO = "lego"
    HAM_RADIO = "ham_radio"
    QUANT = "quant"
    FRANCHISE = "franchise"
    RESTAURANT = "restaurant"
    PROPERTY_MGMT = "property_mgmt"
    PLUMBING = "plumbing"
    ELECTRICAL_TRADE = "electrical_trade"
    HVAC = "hvac"
    MOVING = "moving"
    DECLUTTER = "declutter"
    DIGITAL_NOMAD = "digital_nomad"
    EXPAT = "expat"
    NEURODIVERSITY = "neurodiversity"
    DISABILITY = "disability"
    PHYSICAL_THERAPY = "physical_therapy"
    OPTOMETRY = "optometry"
    GENETICS = "genetics"
    BIOTECH = "biotech"
    SPACEFLIGHT = "spaceflight"
    URBAN_PLANNING = "urban_planning"
    DJ = "dj"
    GUITAR = "guitar"
    PIANO = "piano"
    SINGING = "singing"
    IMPROV = "improv"
    FACILITATION = "facilitation"
    UNION = "union"
    CAMPAIGN = "campaign"
    COCKTAILS = "cocktails"
    FERMENTATION = "fermentation"
    FORAGING = "foraging"
    MYCOLOGY = "mycology"
    PERMACULTURE = "permaculture"
    TINY_HOME = "tiny_home"
    HOME_THEATER = "home_theater"
    STREAMING = "streaming"
    OPEN_SOURCE = "open_source"
    DOCUMENTATION_SITE = "documentation_site"
    OBSERVABILITY = "observability"
    PLATFORM_ENG = "platform_eng"
    PRODUCT_MARKETING = "product_marketing"
    COPYWRITING = "copywriting"
    AFFILIATE = "affiliate"
    AMAZON_FBA = "amazon_fba"
    ETSY = "etsy"
    GRANT_WRITING = "grant_writing"
    BOARD_GOVERNANCE = "board_governance"
    HIGHER_ED = "higher_ed"
    SPECIAL_ED = "special_ed"
    ESL = "esl"
    MEETING = "meeting"
    OKRS = "okrs"
    CHANGE_MGMT = "change_mgmt"
    VC = "vc"
    CROWDFUNDING = "crowdfunding"
    DISASTER_PREP = "disaster_prep"
    HUMANITARIAN = "humanitarian"
    LOCAL_GOV = "local_gov"
    HOUSING = "housing"
    FOOD_SECURITY = "food_security"
    ZERO_WASTE = "zero_waste"
    COMPOSTING = "composting"
    SOLAR_HOME = "solar_home"
    EV = "ev"
    MOTORSPORTS = "motorsports"
    SKATEBOARDING = "skateboarding"
    SURFING = "surfing"
    KAYAKING = "kayaking"
    ROWING = "rowing"
    TRIATHLON = "triathlon"
    POWERLIFTING = "powerlifting"
    BODYBUILDING = "bodybuilding"
    CALISTHENICS = "calisthenics"
    PARKOUR = "parkour"
    # Wave 7 — sports, trades, health, tech, life domains
    TENNIS = "tennis"
    BASKETBALL = "basketball"
    SOCCER = "soccer"
    BASEBALL = "baseball"
    HOCKEY = "hockey"
    VOLLEYBALL = "volleyball"
    BOXING = "boxing"
    WRESTLING = "wrestling"
    FENCING = "fencing"
    ARCHERY = "archery"
    SAILING = "sailing"
    HIKING = "hiking"
    CAMPING = "camping"
    BACKPACKING = "backpacking"
    CROSSFIT = "crossfit"
    PILATES = "pilates"
    GYMNASTICS = "gymnastics"
    ICE_SKATING = "ice_skating"
    OLYMPIC_LIFTING = "olympic_lifting"
    DRUMS = "drums"
    BASS = "bass"
    VIOLIN = "violin"
    MUSIC_PRODUCTION = "music_production"
    SOUND_DESIGN = "sound_design"
    VOICEOVER = "voiceover"
    SCREENWRITING = "screenwriting"
    NOVEL = "novel"
    BLOGGING = "blogging"
    JOURNALING = "journaling"
    TRANSLATION = "translation"
    SIGN_LANGUAGE = "sign_language"
    CROCHET = "crochet"
    EMBROIDERY = "embroidery"
    QUILTING = "quilting"
    COSPLAY = "cosplay"
    MAGIC_TRICKS = "magic_tricks"
    MODEL_BUILDING = "model_building"
    LANDSCAPING = "landscaping"
    ROOFING = "roofing"
    PAINTING_TRADE = "painting_trade"
    FLOORING = "flooring"
    CARPENTRY = "carpentry"
    APPLIANCE_REPAIR = "appliance_repair"
    PEST_CONTROL = "pest_control"
    AUTO_BODY = "auto_body"
    DOG_TRAINING = "dog_training"
    CAT_CARE = "cat_care"
    CHICKENS = "chickens"
    REPTILES = "reptiles"
    CAREGIVING = "caregiving"
    CHRONIC_ILLNESS = "chronic_illness"
    MASSAGE = "massage"
    MENTAL_FITNESS = "mental_fitness"
    FERTILITY = "fertility"
    LACTATION = "lactation"
    PSYCHOLOGY = "psychology"
    NEUROSCIENCE = "neuroscience"
    ECONOMICS = "economics"
    SOCIOLOGY = "sociology"
    ANTHROPOLOGY = "anthropology"
    MATERIALS_SCIENCE = "materials_science"
    ECOLOGY = "ecology"
    PERSONAL_FINANCE = "personal_finance"
    RETIREMENT = "retirement"
    ESTATE_PLANNING = "estate_planning"
    SIDE_HUSTLE = "side_hustle"
    REAL_ESTATE_INVESTING = "real_estate_investing"
    IMPORT_EXPORT = "import_export"
    INVENTORY = "inventory"
    DATA_ENGINEERING = "data_engineering"
    SPREADSHEETS = "spreadsheets"
    NOCODE = "nocode"
    WORDPRESS = "wordpress"
    PRIVACY = "privacy"
    PROMPT_ENG = "prompt_eng"
    KUBERNETES = "kubernetes"
    GRAPHICS_PROG = "graphics_prog"
    COMPILER = "compiler"
    API_DESIGN = "api_design"
    FRONTEND = "frontend"
    BACKEND = "backend"
    PARENTING_TEENS = "parenting_teens"
    ADOPTION = "adoption"
    DIVORCE = "divorce"
    GRIEF = "grief"
    MINIMALISM = "minimalism"
    LUXURY = "luxury"
    THRIFTING = "thrifting"
    ROAD_TRIP = "road_trip"
    CRUISE = "cruise"
    FOOD_TRAVEL = "food_travel"
    TUTORING = "tutoring"
    CURRICULUM = "curriculum"
    EARLY_CHILDHOOD = "early_childhood"
    MONTESSORI = "montessori"
    EDTECH = "edtech"
    NEIGHBORHOOD = "neighborhood"
    VOLUNTEERING = "volunteering"
    FUNDRAISING_EVENTS = "fundraising_events"
    PHOTOGRAPHY_EDITING = "photography_editing"
    VIDEO_EDITING = "video_editing"
    PODCAST_EDITING = "podcast_editing"
    NEWSLETTER = "newsletter"
    COMMUNITY_MGMT = "community_mgmt"
    CUSTOMER_SUCCESS = "customer_success"
    REVENUE_OPS = "revenue_ops"
    PEOPLE_OPS = "people_ops"
    OFFICE_ADMIN = "office_admin"
    RESEARCH_METHODS = "research_methods"
    STATISTICS_APPLIED = "statistics_applied"
    CLIMATE_ACTION = "climate_action"
    RECYCLING = "recycling"
    WATER_CONSERVATION = "water_conservation"
    HOME_SECURITY = "home_security"
    CYBER_HYGIENE = "cyber_hygiene"
    PASSWORD_SECURITY = "password_security"
    BROWSER_EXT = "browser_ext"
    EMAIL_PRODUCTIVITY = "email_productivity"
    NOTE_TAKING = "note_taking"
    SPEED_READING = "speed_reading"
    DEBATE = "debate"
    PUBLIC_POLICY_ANALYSIS = "public_policy_analysis"
    MAPS_GIS = "maps_gis"
    CARTOGRAPHY = "cartography"
    ASTROPHOTOGRAPHY = "astrophotography"
    METEOROLOGY_HOBBY = "meteorology_hobby"
    AMATEUR_ASTRONOMY = "amateur_astronomy"
    BOARD_GAMES = "board_games"
    PUZZLES = "puzzles"
    RUBIKS = "rubiks"
    ORIGAMI = "origami"
    KNIFE_SKILLS = "knife_skills"
    MEAL_PREP = "meal_prep"
    KETO = "keto"
    VEGAN_COOKING = "vegan_cooking"
    GLUTEN_FREE = "gluten_free"
    SOUS_VIDE = "sous_vide"
    SMOKING_MEAT = "smoking_meat"
    # Wave 8 — sports, languages, food, trades, tech, life
    PICKLEBALL = "pickleball"
    BADMINTON = "badminton"
    TABLE_TENNIS = "table_tennis"
    RUGBY = "rugby"
    CRICKET = "cricket"
    SOFTBALL = "softball"
    LACROSSE = "lacrosse"
    WATER_POLO = "water_polo"
    DIVING_SPORT = "diving_sport"
    SYNCHRONIZED_SWIM = "synchronized_swim"
    EQUESTRIAN_SPORT = "equestrian_sport"
    ESPORTS = "esports"
    SPEEDRUNNING = "speedrunning"
    YOGA_THERAPY = "yoga_therapy"
    MOBILITY = "mobility"
    BREATHWORK = "breathwork"
    SPANISH = "spanish"
    FRENCH = "french"
    GERMAN = "german"
    JAPANESE = "japanese"
    MANDARIN = "mandarin"
    KOREAN = "korean"
    ITALIAN = "italian"
    PORTUGUESE = "portuguese"
    ARABIC = "arabic"
    HINDI = "hindi"
    GREEK_LANG = "greek_lang"
    LATIN = "latin"
    UKULELE = "ukulele"
    SAXOPHONE = "saxophone"
    TRUMPET = "trumpet"
    FLUTE = "flute"
    HARMONICA = "harmonica"
    BANJO = "banjo"
    MUSIC_THEORY = "music_theory"
    GRAPHIC_DESIGN = "graphic_design"
    ILLUSTRATION = "illustration"
    UX_WRITING = "ux_writing"
    STORYBOARD = "storyboard"
    COLOR_GRADING = "color_grading"
    LIGHTING_DESIGN = "lighting_design"
    COSTUME_DESIGN = "costume_design"
    SET_DESIGN = "set_design"
    PASTRY = "pastry"
    BREAD = "bread"
    CHOCOLATE = "chocolate"
    CHEESE = "cheese"
    CHARCUTERIE = "charcuterie"
    PRESERVING = "preserving"
    INDIAN_COOKING = "indian_cooking"
    CHINESE_COOKING = "chinese_cooking"
    MEXICAN_COOKING = "mexican_cooking"
    ITALIAN_COOKING = "italian_cooking"
    JAPANESE_COOKING = "japanese_cooking"
    BBQ_SAUCES = "bbq_sauces"
    COFFEE_ROASTING = "coffee_roasting"
    LATTE_ART = "latte_art"
    HOUSEPLANTS = "houseplants"
    HYDROPONICS = "hydroponics"
    BONSAI = "bonsai"
    AQUAPONICS = "aquaponics"
    LAWN_CARE = "lawn_care"
    IRRIGATION = "irrigation"
    POOL_CARE = "pool_care"
    FIREPLACE = "fireplace"
    DENTAL_HYGIENE = "dental_hygiene"
    PHARMACOLOGY = "pharmacology"
    RADIOLOGY_LITERACY = "radiology_literacy"
    NUTRITION_SCIENCE = "nutrition_science"
    EPIDEMIOLOGY = "epidemiology"
    BIOSTATISTICS = "biostatistics"
    BOOKKEEPING = "bookkeeping"
    PAYROLL = "payroll"
    BILLING = "billing"
    PRICING = "pricing"
    SALES_ENABLEMENT = "sales_enablement"
    PARTNERSHIPS = "partnerships"
    CUSTOMER_RESEARCH = "customer_research"
    ANALYTICS = "analytics"
    AB_TESTING = "ab_testing"
    RUST_LANG = "rust_lang"
    GO_LANG = "go_lang"
    PYTHON_DATA = "python_data"
    SQL_ANALYTICS = "sql_analytics"
    TERRAFORM = "terraform"
    ANSIBLE = "ansible"
    CICD = "cicd"
    DOCKER = "docker"
    LINUX_ADMIN = "linux_admin"
    NETWORK_SECURITY = "network_security"
    PENTEST_DEFENSE = "pentest_defense"
    THREAT_MODEL = "threat_model"
    INCIDENT_RESPONSE = "incident_response"
    QA_TESTING = "qa_testing"
    MOBILE_QA = "mobile_qa"
    ACCESSIBILITY_ENG = "accessibility_eng"
    PERFORMANCE_WEB = "performance_web"
    SEO_TECHNICAL = "seo_technical"
    TAX_PREP = "tax_prep"
    INSURANCE_CLAIMS = "insurance_claims"
    CAR_BUYING = "car_buying"
    HOME_BUYING = "home_buying"
    RENTING = "renting"
    COLLEGE_APPS = "college_apps"
    SCHOLARSHIPS = "scholarships"
    STUDY_ABROAD = "study_abroad"
    INTERNSHIP = "internship"
    CAREER_CHANGE = "career_change"
    LINKEDIN = "linkedin"
    WHATSAPP = "whatsapp"
    NETWORKING_CAREER = "networking_career"
    HOA_LIVING = "hoa_living"
    COOP_HOUSING = "coop_housing"
    COMMUNITY_GARDEN = "community_garden"
    MUTUAL_AID = "mutual_aid"
    FISHKEEPING = "fishkeeping"
    TERRARIUM = "terrarium"
    ANTKEEPING = "antkeeping"
    BEEKEEPING_ADVANCED = "beekeeping_advanced"
    FOUNTAIN_PEN = "fountain_pen"
    STATIONERY = "stationery"
    MECHANICAL_KEYBOARD = "mechanical_keyboard"
    PC_BUILDING = "pc_building"
    HOME_LAB = "home_lab"
    THREE_D_MODELING = "three_d_modeling"
    CNC = "cnc"
    LASER_CUTTING = "laser_cutting"
    RESIN_PRINTING = "resin_printing"
    FILAMENT_PRINTING = "filament_printing"
    MEDITATION = "meditation"
    STOICISM = "stoicism"
    JOURNAL_PROMPTS = "journal_prompts"
    HABIT_BUILDING = "habit_building"
    TIME_BLOCKING = "time_blocking"
    SECOND_BRAIN = "second_brain"
    PACKING = "packing"
    TRAVEL_PHOTOGRAPHY = "travel_photography"
    SOLO_TRAVEL = "solo_travel"
    FAMILY_TRAVEL = "family_travel"
    BUDGET_TRAVEL = "budget_travel"
    POINTS_MILES = "points_miles"
    REAL_ESTATE_PHOTO = "real_estate_photo"
    STAGING = "staging"
    INTERIOR_STYLING = "interior_styling"
    EVENT_PHOTOGRAPHY = "event_photography"
    PORTRAIT_PHOTO = "portrait_photo"
    STREET_PHOTO = "street_photo"
    WILDLIFE_PHOTO = "wildlife_photo"
    ASTRO_IMAGING_PROC = "astro_imaging_proc"

# Chat helpers that adapt tone from the ask (Answers window / headless ask).
SHAPED_TOOLS = frozenset({
    "claude",
    "grok",
    "gemini",
    "codex",
    "copilot",
    "antigravity",
    "aider",
})


# Strong signals that someone already thinks in code.
_CODE_WORDS = frozenset(
    {
        # languages / runtimes
        "python",
        "rust",
        "typescript",
        "javascript",
        "java",
        "golang",
        "kotlin",
        "swift",
        "ruby",
        "php",
        "sql",
        "html",
        "css",
        "react",
        "vue",
        "svelte",
        "node",
        "nodejs",
        "npm",
        "pnpm",
        "yarn",
        "cargo",
        "pip",
        "pytest",
        "jest",
        "vite",
        "webpack",
        "django",
        "flask",
        "fastapi",
        "rails",
        # tools / workflow
        "git",
        "github",
        "gitlab",
        "commit",
        "commits",
        "branch",
        "rebase",
        "merge",
        "pr",
        "prs",
        "diff",
        "diffs",
        "patch",
        "patches",
        "ci",
        "cd",
        "dockerfile",
        "docker",
        "kubernetes",
        "k8s",
        "lint",
        "linter",
        "linters",
        "eslint",
        "clippy",
        "formatter",
        "refactor",
        "refactors",
        "refactored",
        "refactoring",
        # code nouns / verbs
        "function",
        "functions",
        "method",
        "methods",
        "class",
        "classes",
        "struct",
        "structs",
        "trait",
        "traits",
        "interface",
        "interfaces",
        "enum",
        "enums",
        "type",
        "types",
        "typedef",
        "generics",
        "async",
        "await",
        "promise",
        "callback",
        "closure",
        "module",
        "modules",
        "import",
        "imports",
        "export",
        "exports",
        "package",
        "packages",
        "dependency",
        "dependencies",
        "api",
        "apis",
        "endpoint",
        "endpoints",
        "schema",
        "schemas",
        "query",
        "queries",
        "mutex",
        "thread",
        "threads",
        "race",
        "deadlock",
        "stacktrace",
        "traceback",
        "backtrace",
        "segfault",
        "null",
        "nil",
        "undefined",
        "exception",
        "exceptions",
        "error",
        "errors",
        "typeerror",
        "valueerror",
        "keyerror",
        "indexerror",
        "attributeerror",
        "runtimeerror",
        "nameerror",
        "importerror",
        "syntaxerror",
        "assertionerror",
        "nullptr",
        "nullpointer",
        "segfault",
        "panic",
        "unwrap",
        "bug",
        "bugs",
        "unit",
        "integration",
        "mock",
        "mocks",
        "stub",
        "stubs",
        "fixture",
        "fixtures",
        "compile",
        "compiler",
        "runtime",
        "binary",
        "binaries",
        "argv",
        "stdin",
        "stdout",
        "stderr",
        "regex",
        "regexp",
        "json",
        "yaml",
        "toml",
        "protobuf",
        "grpc",
        "oauth",
        "jwt",
        "http",
        "https",
        "websocket",
        "tcp",
        "udp",
        "cli",
        "sdk",
        "ide",
        "lsp",
        "parser",
        "lexer",
        "ast",
        "bytecode",
        "heap",
        "stack",
        "malloc",
        "pointer",
        "pointers",
        "reference",
        "ownership",
        "borrow",
        "lifetime",
        "lifetimes",
        "impl",
        "fn",
        "pub",
        "const",
        "let",
        "var",
        "def",
        "lambda",
        "decorator",
        "decorators",
        "macro",
        "macros",
        "pragma",
        "makefile",
        "cmake",
        "bazel",
        "monorepo",
        "repo",
        "repos",
        "codebase",
        "src",
        "lib",
        "crate",
        "crates",
        "pyproject",
        "cargo.toml",
        "package.json",
        "tsconfig",
        "webpack",
        "vite.config",
        "pull-request",
        "pullrequest",
        "code-review",
        "codereview",
        "typecheck",
        "type-check",
        "unit-test",
        "unit-tests",
        "e2e",
        "smoke-test",
        "regression",
        "hotfix",
        "cherry-pick",
        "cherrypick",
        "squash",
        "bisect",
        "blame",
        "checkout",
        "stash",
        "upstream",
        "downstream",
        "fork",
        "clone",
        "sha",
        "hash",
        "uuid",
        "serde",
        "tokio",
        "axum",
        "actix",
        "pytorch",
        "tensorflow",
        "numpy",
        "pandas",
        "sqlalchemy",
        "postgres",
        "postgresql",
        "mongodb",
        "redis",
        "sqlite",
        "graphql",
        "rest",
        "openapi",
        "swagger",
        "middleware",
        "handler",
        "handlers",
        "controller",
        "controllers",
        "service",
        "services",
        "repository",
        "dto",
        "orm",
        "mvc",
        "mvvm",
        "spa",
        "ssr",
        "csr",
        "dom",
        "jsx",
        "tsx",
        "webpack",
        "bundler",
        "transpile",
        "transpile",
        "polyfill",
        "shim",
        "ffi",
        "wasm",
        "webassembly",
        "kernel",
        "syscall",
        "mmap",
        "ioctl",
        "qemu",
        "hypervisor",
        "dockerfile",
        "compose.yaml",
        "kustomize",
        "helm",
        "terraform",
        "ansible",
        "ci/cd",
        "github-actions",
        "gitlab-ci",
    }
)

# Everyday wording that should keep us in plain mode even if a weak code word slips in.
_PLAIN_PHRASES = (
    "in simple words",
    "in plain english",
    "in plain language",
    "i'm not a developer",
    "i am not a developer",
    "i'm not technical",
    "i am not technical",
    "don't know code",
    "dont know code",
    "don't know how to code",
    "no coding",
    "not a programmer",
    "explain like i'm five",
    "explain like im five",
    "eli5",
    "for beginners",
    "i don't understand code",
    "i dont understand code",
)

# Code-ish tokens: foo(), path/to/file.ext, snake_case identifiers with _, ::, ->
_CODE_TOKEN_RE = re.compile(
    r"(?:"
    r"\b[\w./-]+\.(?:py|rs|ts|tsx|js|jsx|go|java|kt|swift|rb|php|c|cc|cpp|h|hpp|cs|sh|bash|zsh|toml|yaml|yml|json|md|sql|css|scss|html|vue|svelte|proto|gradle|cmake|mk)\b"
    r"|`[^`]+`"
    r"|\b[A-Za-z_][\w]*\s*\("
    r"|\b[a-z][a-z0-9]*(?:_[a-z0-9]+)+\b"
    r"|::|->|=>|</?[A-Za-z]"
    r"|\$\w+"
    r"|\b(?:PR|MR)\s*#?\d+"
    r")",
    re.I,
)

# "fix the X function", "change this method", "implement Y"
_CODE_INTENT_RE = re.compile(
    r"\b(?:"
    r"implement|refactor|typecheck|type-check|unit[- ]?test|"
    r"fix(?:es|ing)?\s+(?:the\s+)?(?:bug|error|issue|function|method|class|api|endpoint|"
    r"typeerror|valueerror|exception|stacktrace|traceback)|"
    r"fix(?:es|ing)?\s+(?:the\s+)?\w*error\b|"
    r"add\s+(?:a\s+)?(?:test|tests|unit test|endpoint|handler|function|method)|"
    r"change\s+(?:the\s+)?(?:function|method|class|api|signature|return type)|"
    r"write\s+(?:a\s+)?(?:function|method|class|test|script|patch)|"
    r"(?:the\s+)?(?:function|method|class|handler|endpoint)\b"
    r")\b",
    re.I,
)


def _tokens(text: str) -> set[str]:
    # Split on non-alnum but keep dots for package.json style (handled separately).
    parts = re.findall(r"[A-Za-z][A-Za-z0-9+./#_-]*", text.lower())
    out: set[str] = set()
    for p in parts:
        out.add(p)
        # package.json → package, json
        if "." in p:
            out.update(p.split("."))
        if "/" in p:
            out.update(p.split("/"))
        if "-" in p:
            out.update(p.split("-"))
        if "_" in p:
            out.update(p.split("_"))
    return out


# --- Non-code persona lexicons ----------------------------------------------

_ACADEMIC_WORDS = frozenset({
    "thesis", "dissertation", "abstract", "methodology", "literature",
    "citation", "citations", "cite", "bibliography", "footnote", "footnotes",
    "peer", "review", "peer-reviewed", "journal", "journals", "manuscript",
    "hypothesis", "epistemology", "ontology", "qualitative", "quantitative",
    "ethnography", "phenomenology", "apa", "mla", "chicago", "harvard",
    "doi", "isbn", "preprint", "arxiv", "scholar", "academia", "academic",
    "professor", "faculty", "tenure", "conference", "symposium", "proceedings",
    "rigor", "rigour", "framework", "theoretical", "empirical",
    "replication", "validity", "reliability", "sampling", "correlational",
    "anova", "pvalue", "p-value", "significance",
    "argument", "claim", "counterargument", "synthesis", "critical",
    "historiography", "hermeneutic", "discourse", "positionality",
})
_ACADEMIC_PHRASES = (
    "literature review", "peer review", "research paper", "research question",
    "theoretical framework", "in-text citation", "works cited", "annotated bibliography",
    "journal article", "call for papers", "revise and resubmit", "impact factor",
    "systematic review", "meta-analysis", "grounded theory", "case study",
)

_STUDENT_WORDS = frozenset({
    "homework", "assignment", "essay", "midterm", "finals", "exam", "exams",
    "quiz", "coursework", "syllabus", "lecture", "lectures", "semester",
    "gpa", "grade", "grades", "rubric", "classmate", "classmates",
    "campus", "dorm", "freshman", "sophomore", "junior", "senior", "undergrad",
    "undergraduate", "grad", "graduate", "student", "students", "school",
    "college", "university", "uni", "tutor", "tutoring", "study", "studying",
    "notes", "flashcards", "cram", "due", "deadline", "plagiarism", "turnitin",
    "canvas", "blackboard", "moodle", "worksheet", "handout", "ta",
})
_STUDENT_PHRASES = (
    "help me study", "for my class", "my professor", "my teacher", "my homework",
    "my assignment", "due tomorrow", "due tonight", "extra credit", "study guide",
    "practice problems", "i don't understand", "i dont understand",
    "for school", "group project", "lab report", "office hours", "problem set",
)

_WRITER_WORDS = frozenset({
    "blog", "blogging", "newsletter", "novel", "novella", "chapter", "chapters",
    "draft", "redraft", "rewrite", "rewriting", "prose", "poem", "poetry",
    "verse", "stanza", "dialogue", "narrator", "voice", "tone",
    "copywriting", "headline", "tagline", "slogan", "caption",
    "op-ed", "oped", "editorial", "memoir", "screenplay", "script", "scene",
    "character", "plot", "pacing", "proofread", "proofreading",
    "copyedit", "line-edit", "grammar", "punctuation",
    "hook", "lede", "byline", "publish", "publishing", "substack",
    "ghostwrite", "ghostwriting",
})
_WRITER_PHRASES = (
    "make this shorter", "make this longer", "more professional", "more casual",
    "change the tone", "tighten the prose", "active voice", "passive voice",
    "opening paragraph", "closing paragraph", "blog post",
    "call to action",
)

_BUSINESS_WORDS = frozenset({
    "stakeholder", "stakeholders", "kpi", "kpis", "okr", "okrs", "roi",
    "roadmap", "roadmap", "sprint", "backlog", "priority", "prioritize",
    "deck", "slides", "powerpoint", "keynote", "memo", "brief",
    "budget", "forecast", "revenue", "margin", "churn", "pipeline",
    "sales", "crm", "ops", "operations", "process", "workflow",
    "meeting", "agenda", "minutes", "action-items", "actionitems",
    "vendor", "rfp", "proposal", "procurement", "compliance",
    "headcount", "org", "org-chart", "orgchart", "hire", "hiring",
    "performance", "review", "1:1", "one-on-one", "manager", "leadership",
    "strategy", "strategic", "initiative", "deliverable", "milestone",
    "qbr", "board", "exec", "executive", "ceo", "cfo", "coo", "cmo",
})
_BUSINESS_PHRASES = (
    "status update", "project plan", "go to market", "go-to-market",
    "business case", "value prop", "value proposition", "competitive analysis",
    "swot", "north star", "key results", "slide deck", "for leadership",
    "for the team", "stakeholder email", "weekly update",
)

_DESIGN_WORDS = frozenset({
    "figma", "sketch", "adobe", "photoshop", "illustrator", "indesign",
    "mockup", "mockups", "wireframe", "wireframes", "prototype", "prototypes",
    "ui", "ux", "uxui", "interface", "layout", "typography", "typeface",
    "font", "fonts", "palette", "color", "colours", "contrast", "spacing",
    "margin", "padding", "grid", "icon", "icons", "illustration",
    "brand", "branding", "logo", "logotype", "visual", "aesthetic",
    "accessibility", "a11y", "wcag", "responsive", "mobile-first",
    "design-system", "designsystem", "component", "components", "token",
    "tokens", "dark-mode", "light-mode", "affordance", "usability",
    "heuristic", "persona", "journey", "storyboard",
})
_DESIGN_PHRASES = (
    "user flow", "design critique", "visual hierarchy", "color palette",
    "style guide", "brand guidelines", "landing page design", "mobile ui",
    "desktop ui", "make it prettier", "looks off", "spacing is wrong",
)

_JOB_WORDS = frozenset({
    "resume", "cv", "portfolio", "interview", "interviews",
    "interviewer", "recruiter", "recruiting", "job", "jobs", "career",
    "careers", "application", "offer", "salary", "compensation",
    "negotiation", "negotiating", "referral", "hiring", "candidate",
    "ats", "internship", "intern", "fellowship", "layoff", "laid-off",
    "job-hunt", "jobhunt", "headhunter",
    # "linkedin" alone is NOT a job signal — messaging vs profile/resume
    # are disambiguated below; bare linkedin belongs to Audience.LINKEDIN.
})
_JOB_PHRASES = (
    "cover letter", "personal statement", "statement of purpose",
    "job description", "job posting", "tell me about yourself",
    "behavioral interview", "system design interview", "salary negotiation",
    "why should we hire", "strengths and weaknesses", "thank you email",
    "follow up email", "linkedin summary", "resume bullet",
    "linkedin profile for job", "update my resume", "job application",
)

_TEACHER_WORDS = frozenset({
    "lesson", "lessons", "curriculum", "pedagogy", "pedagogical",
    "classroom", "pupils", "learners", "worksheet", "rubric",
    "differentiate", "differentiation", "scaffolding", "formative",
    "summative", "assessment", "assessments", "iep", "slo",
    "lesson-plan", "lessonplan", "unit-plan", "unitplan",
    "teaching", "teach", "educator", "instructor", "faculty",
})
_TEACHER_PHRASES = (
    "lesson plan", "for my students", "for the class", "learning objective",
    "learning outcomes", "exit ticket", "warm up activity", "explain to kids",
    "grade level", "year 7", "year 8", "9th grade", "10th grade",
    "differentiated instruction", "classroom activity",
)

_DATA_WORDS = frozenset({
    "spreadsheet", "excel", "sheets", "csv", "tsv", "dataframe",
    "pivot", "chart", "charts", "graph", "graphs", "dashboard",
    "visualization", "visualisation", "histogram", "scatter",
    "correlation", "regression", "outlier", "outliers", "mean",
    "median", "percentile", "cohort", "funnel", "conversion",
    "analytics", "metric", "metrics", "kpi", "dataset", "datasets",
    "sql", "query", "queries", "tableau", "looker", "powerbi",
    "power-bi", "bi", "etl", "warehouse", "bigquery", "snowflake",
    "ab-test", "abtest", "a/b", "significance", "sample-size",
})
_DATA_PHRASES = (
    "data analysis", "clean this data", "pivot table", "excel formula",
    "google sheets", "make a chart", "interpret these numbers",
    "what does this mean statistically", "sql query", "group by",
)

_FOUNDER_WORDS = frozenset({
    "startup", "start-up", "founder", "cofounder", "co-founder",
    "mvp", "pmf", "product-market", "fundraising", "fundraise",
    "seed", "series-a", "seriesa", "angel", "vc", "vcs", "investor",
    "investors", "pitch", "runway", "burn", "cap-table", "captable",
    "equity", "saas", "b2b", "b2c", "gtm", "plg", "bootstrap",
    "bootstrapped", "ycombinator", "y-combinator", "accelerator",
    "incubator", "term-sheet", "termsheet", "valuation",
})
_FOUNDER_PHRASES = (
    "pitch deck", "raise money", "product market fit", "go to market",
    "landing page copy", "for investors", "cold email investors",
    "startup idea", "validate the idea", "build an mvp",
)

_LEGAL_WORDS = frozenset({
    "contract", "contracts", "nda", "tos", "terms", "privacy",
    "gdpr", "ccpa", "clause", "clauses", "liability", "indemnity",
    "indemnification", "warranty", "warranties", "jurisdiction",
    "arbitration", "litigation", "attorney", "lawyer", "counsel",
    "compliance", "regulatory", "statute", "statutory", "license",
    "licence", "copyright", "trademark", "patent", "ip",
    "non-compete", "noncompete", "non-solicit", "confidentiality",
})
_LEGAL_PHRASES = (
    "terms of service", "privacy policy", "non disclosure", "non-disclosure",
    "service agreement", "not legal advice", "review this contract",
    "redline this", "plain english legal", "explain this clause",
)

_PARENT_WORDS = frozenset({
    "kid", "kids", "child", "children", "toddler", "teen", "teenager",
    "daughter", "son", "parent", "parents", "bedtime", "homework",
    "schoolwork", "family", "babysit", "babysitter", "pediatric",
    "kindergarten", "preschool", "grade-school",
})
_PARENT_PHRASES = (
    "for my kid", "for my child", "for my daughter", "for my son",
    "explain to a child", "explain to my kid", "age appropriate",
    "5 year old", "6 year old", "7 year old", "10 year old",
    "bedtime story", "help my kid with",
)

_MARKETING_WORDS = frozenset({
    "campaign", "campaigns", "ads", "ad", "advertising", "advert",
    "seo", "sem", "ppc", "cpc", "cpm", "roas", "cac", "ltv",
    "funnel", "nurture", "newsletter", "drip", "utm", "landing",
    "brand", "positioning", "messaging", "creative", "copydeck",
    "social", "instagram", "tiktok", "twitter", "linkedin", "content",
    "influencer", "retargeting", "remarketing", "conversion", "cta",
    "abm", "growth", "growth-hacking", "virality", "impressions",
})
_MARKETING_PHRASES = (
    "marketing campaign", "ad copy", "social media post", "email campaign",
    "landing page copy", "seo title", "meta description", "content calendar",
    "brand voice", "go-to-market campaign", "paid social",
)

_SALES_WORDS = frozenset({
    "outbound", "inbound", "prospect", "prospects", "lead", "leads",
    "pipeline", "quota", "quota", "deal", "deals", "crm", "salesforce",
    "hubspot", "cold", "outbound", "discovery", "demo", "demos",
    "objection", "objections", "closing", "close", "ae", "sdr", "bdr",
    "upsell", "cross-sell", "renewal", "churn", "arr", "mrr",
})
_SALES_PHRASES = (
    "cold email", "cold call", "sales call", "discovery call",
    "follow up email", "objection handling", "sales pipeline",
    "qualify the lead", "book a demo", "close the deal",
)

_FINANCE_WORDS = frozenset({
    "accounting", "bookkeeping", "bookkeeper", "invoice", "invoices",
    "receivable", "payable", "p&l", "pnl", "balance-sheet", "cashflow",
    "cash-flow", "budget", "forecast", "forecasting", "tax", "taxes",
    "irs", "vat", "gaap", "ifrs", "audit", "ledger", "reconciliation",
    "expense", "expenses", "payroll", "capex", "opex", "ebitda",
    "amortization", "depreciation", "equity", "debt", "interest",
    "mortgage", "investment", "portfolio", "401k", "ira", "dividend",
})
_FINANCE_PHRASES = (
    "profit and loss", "cash flow", "tax return", "expense report",
    "financial model", "balance sheet", "accounts receivable",
    "personal finance", "monthly budget", "reconcile the books",
)

_PRODUCT_WORDS = frozenset({
    "roadmap", "backlog", "user-story", "userstory", "epic", "prd",
    "spec", "specs", "requirements", "acceptance", "prioritization",
    "rice", "moscow", "mvp", "beta", "feature", "features", "release",
    "changelog", "feedback", "usability", "persona", "personas",
    "jtbd", "jobs-to-be-done", "north-star", "activation", "retention",
    "onboarding", "pm", "product", "discovery", "experiment",
})
_PRODUCT_PHRASES = (
    "product requirements", "product roadmap", "user story",
    "product manager", "feature prioritization", "product spec",
    "acceptance criteria", "customer interview", "jobs to be done",
)

_SUPPORT_WORDS = frozenset({
    "ticket", "tickets", "zendesk", "intercom", "freshdesk", "helpdesk",
    "help-desk", "support", "customer", "csat", "nps", "sla",
    "escalation", "refund", "cancel", "cancellation", "bug-report",
    "troubleshooting", "faq", "kb", "knowledge-base", "macros",
    "agent", "agents", "inbox", "reply", "replies",
})
_SUPPORT_PHRASES = (
    "customer support", "support ticket", "help article", "help center",
    "reply to customer", "canned response", "troubleshoot this",
    "customer complained", "refund request", "how do i fix",
)

_SCIENCE_WORDS = frozenset({
    "experiment", "experiments", "lab", "laboratory", "protocol",
    "reagent", "assay", "microscope", "specimen", "sample",
    "physics", "chemistry", "biology", "genomics", "proteomics",
    "molecule", "atom", "reaction", "equation", "hypothesis",
    "control-group", "placebo", "double-blind", "peer-lab",
    "spectrometer", "pcr", "crispr", "genome", "protein",
    "simulation", "model", "units", "si", "calibration",
})
_SCIENCE_PHRASES = (
    "lab report", "experimental design", "control group", "lab protocol",
    "scientific method", "replicate the experiment", "physics problem",
    "chemistry problem", "balance this equation",
)

_LANGUAGE_WORDS = frozenset({
    "translate", "translation", "translator", "bilingual", "multilingual",
    "spanish", "french", "german", "italian", "portuguese", "chinese",
    "mandarin", "japanese", "korean", "arabic", "hindi", "greek",
    "esl", "efl", "grammar", "vocabulary", "pronunciation", "fluent",
    "fluency", "conjugation", "conjugations", "dialect", "idiom",
    "idioms", "duolingo", "anki", "polyglot",
})
_LANGUAGE_PHRASES = (
    "translate this", "learn spanish", "learn french", "english as a second",
    "practice speaking", "is this grammar correct", "how do you say",
    "native speaker", "language exchange", "improve my english",
)

_CREATIVE_WORDS = frozenset({
    "music", "song", "songs", "lyrics", "melody", "chord", "chords",
    "beat", "tempo", "film", "movie", "cinematic", "storyboard",
    "photography", "photo", "photos", "camera", "lens", "exposure",
    "aperture", "composition", "podcast", "episode", "youtube",
    "video", "videos", "edit", "premiere", "davinci", "ableton",
    "synth", "mix", "mastering", "painting", "draw", "drawing",
    "illustration", "comic", "animation", "vfx", "color-grade",
})
_CREATIVE_PHRASES = (
    "write lyrics", "song structure", "film shot list", "photo composition",
    "video edit", "podcast outline", "creative brief", "album concept",
    "short film", "youtube script",
)

_HEALTH_WORDS = frozenset({
    "symptoms", "symptom", "diagnosis", "treatment", "medication",
    "prescription", "doctor", "clinic", "hospital", "wellness",
    "nutrition", "diet", "workout", "exercise", "sleep", "anxiety",
    "therapy", "therapist", "mental", "fitness", "calorie", "protein",
    "vitamins", "supplement", "injury", "recovery", "physio",
    "pt", "yoga", "meditation", "mindfulness",
})
_HEALTH_PHRASES = (
    "not medical advice", "should i see a doctor", "healthy meal",
    "workout plan", "sleep better", "manage stress", "side effects",
    "general wellness", "fitness routine",
)

_NONPROFIT_WORDS = frozenset({
    "nonprofit", "non-profit", "ngo", "charity", "donation", "donations",
    "donor", "donors", "grant", "grants", "fundraising", "volunteer",
    "volunteers", "501c3", "board", "mission", "impact", "beneficiaries",
    "community", "outreach", "advocacy", "campaign",
})
_NONPROFIT_PHRASES = (
    "grant application", "donor letter", "nonprofit board", "volunteer signup",
    "impact report", "fundraising campaign", "mission statement",
)

_POLICY_WORDS = frozenset({
    "policy", "policies", "legislation", "bill", "regulation", "regulatory",
    "government", "congress", "parliament", "municipal", "civic",
    "public", "constituent", "constituents", "lobby", "lobbying",
    "white-paper", "whitepaper", "briefing", "memo", "ordinance",
    "zoning", "housing", "climate", "immigration", "voting",
})
_POLICY_PHRASES = (
    "policy brief", "public comment", "legislative", "government proposal",
    "civic engagement", "policy memo", "for policymakers",
)

_REAL_ESTATE_WORDS = frozenset({
    "lease", "landlord", "tenant", "rent", "rental", "mortgage", "escrow",
    "closing", "appraisal", "listing", "realtor", "broker", "hoa",
    "condo", "apartment", "property", "real-estate", "realestate",
    "mls", "zillow", "redfin", "square-feet", "sqft", "inspection",
})
_REAL_ESTATE_PHRASES = (
    "real estate", "buy a house", "rent increase", "security deposit",
    "lease agreement", "offer letter home", "home inspection", "open house",
)

_TRAVEL_WORDS = frozenset({
    "itinerary", "flight", "flights", "hotel", "airbnb", "visa", "passport",
    "luggage", "layover", "boarding", "airport", "train", "backpack",
    "travel", "trip", "vacation", "holiday", "tourist", "tourism",
    "booking", "reservation", "jetlag", "customs", "immigration",
})
_TRAVEL_PHRASES = (
    "travel plan", "trip itinerary", "weekend getaway", "packing list",
    "best time to visit", "flight booking", "hotel recommendation",
)

_COOKING_WORDS = frozenset({
    "recipe", "recipes", "cook", "cooking", "bake", "baking", "kitchen",
    "ingredient", "ingredients", "oven", "stove", "simmer", "saute",
    "marinate", "seasoning", "cuisine", "meal", "dinner", "lunch",
    "breakfast", "dessert", "vegan", "vegetarian", "gluten", "kosher",
    "halal", "sous-vide", "meal-prep", "mealprep",
})
_COOKING_PHRASES = (
    "dinner recipe", "how to cook", "meal plan", "substitute ingredient",
    "bake a cake", "weeknight dinner", "shopping list for dinner",
)

_GAMING_WORDS = frozenset({
    "game", "games", "gamer", "gaming", "steam", "xbox", "playstation",
    "nintendo", "npc", "quest", "leveling", "loot", "raid", "mmorpg",
    "fps", "rpg", "esports", "stream", "streaming", "twitch", "discord",
    "mod", "mods", "speedrun", "boss", "multiplayer", "co-op",
    "gameplay", "game-design", "gamedesign", "unity", "unreal",
})
_GAMING_PHRASES = (
    "game design", "build a game", "twitch stream", "raid guide",
    "character build", "how to beat", "gaming setup", "indie game",
)

_SPORTS_WORDS = frozenset({
    "coach", "coaching", "athlete", "training", "workout", "match",
    "tournament", "league", "soccer", "football", "basketball", "baseball",
    "tennis", "golf", "marathon", "sprint", "cardio", "strength",
    "periodization", "playbook", "offense", "defense", "roster",
    "injury", "rehab", "warmup", "cool-down",
})
_SPORTS_PHRASES = (
    "training plan", "game plan", "practice drills", "sports coach",
    "improve my running", "strength training", "team strategy",
)

_HR_WORDS = frozenset({
    "hr", "human-resources", "onboarding", "offboarding", "benefits",
    "payroll", "handbook", "policy", "pip", "performance", "review",
    "compensation", "band", "leveling", "headcount", "requisition",
    "diversity", "inclusion", "dei", "harassment", "workplace",
    "employee", "employees", "manager", "1:1", "skip-level",
})
_HR_PHRASES = (
    "hr policy", "employee handbook", "performance review", "pip letter",
    "job requisition", "benefits enrollment", "workplace investigation",
    "people ops", "human resources",
)

_JOURNALISM_WORDS = frozenset({
    "byline", "lede", "nutgraf", "dateline", "source", "sources",
    "interview", "reporting", "reporter", "editor", "newsroom",
    "scoop", "headline", "subhead", "fact-check", "factcheck",
    "investigative", "press", "media", "op-ed", "column",
})
_JOURNALISM_PHRASES = (
    "news story", "press release", "fact check", "on the record",
    "off the record", "news lede", "journalistic", "for publication",
)

_ACCESSIBILITY_WORDS = frozenset({
    "accessibility", "a11y", "wcag", "screen-reader", "screenreader",
    "aria", "caption", "captions", "subtitle", "subtitles", "alt-text",
    "alttext", "contrast", "keyboard", "focus", "disability",
    "disabilities", "inclusive", "neurodivergent", "blind", "deaf",
    "hard-of-hearing", "motor", "cognitive",
})
_ACCESSIBILITY_PHRASES = (
    "accessible design", "screen reader", "alt text", "wcag compliance",
    "keyboard navigation", "inclusive design", "accessibility audit",
)

_ENGINEERING_WORDS = frozenset({
    "mechanical", "civil", "structural", "electrical", "thermal",
    "cad", "solidworks", "autocad", "tolerances", "torque", "load",
    "stress", "strain", "beam", "circuit", "schematic", "pcb",
    "material", "materials", "welding", "machining", "prototype",
    "fea", "cfd", "blueprint", "drawing", "dimension", "iso",
})
_ENGINEERING_PHRASES = (
    "mechanical design", "circuit design", "load calculation",
    "structural analysis", "engineering drawing", "bill of materials",
    "tolerance stack", "thermal analysis",
)

_SECURITY_WORDS = frozenset({
    "security", "cybersecurity", "infosec", "vulnerability", "cve",
    "exploit", "phishing", "malware", "ransomware", "firewall",
    "auth", "authentication", "authorization", "oauth", "mfa", "2fa",
    "encryption", "tls", "ssl", "pentest", "penetration", "siem",
    "soc", "zero-trust", "zerotrust", "threat", "incident", "forensics",
})
_SECURITY_PHRASES = (
    "security review", "threat model", "penetration test", "incident response",
    "secure this", "password policy", "data breach", "security audit",
)

_HOSPITALITY_WORDS = frozenset({
    "hotel", "restaurant", "menu", "guest", "guests", "hospitality",
    "reservation", "concierge", "front-desk", "frontdesk", "housekeeping",
    "catering", "dining", "service", "sommelier", "chef", "barista",
})
_HOSPITALITY_PHRASES = (
    "hotel operations", "restaurant menu", "guest experience",
    "hospitality training", "front desk", "table service",
)

_EVENTS_WORDS = frozenset({
    "event", "events", "wedding", "conference", "meetup", "venue",
    "rsvp", "catering", "agenda", "run-of-show", "runofshow",
    "speaker", "speakers", "sponsor", "sponsors", "ticketing",
    "seating", "av", "livestream",
})
_EVENTS_PHRASES = (
    "event plan", "wedding plan", "run of show", "conference agenda",
    "speaker lineup", "event budget", "guest list",
)

_FASHION_WORDS = frozenset({
    "fashion", "outfit", "wardrobe", "style", "styling", "runway",
    "collection", "fabric", "textile", "sewing", "tailoring",
    "lookbook", "capsule", "trend", "trends", "couture", "streetwear",
})
_FASHION_PHRASES = (
    "outfit ideas", "what to wear", "fashion collection", "style guide",
    "wardrobe capsule", "look book",
)

_DIY_WORDS = frozenset({
    "diy", "repair", "fix", "broken", "tools", "hammer", "drill",
    "plumbing", "electrical", "paint", "painting", "drywall", "wood",
    "furniture", "assemble", "ikea", "renovation", "remodel",
    "garden", "lawn", "fence", "shelf",
})
_DIY_PHRASES = (
    "how to fix", "home repair", "diy project", "build a shelf",
    "install a", "renovate", "handyman", "home improvement",
)

_ENVIRONMENT_WORDS = frozenset({
    "climate", "carbon", "emissions", "sustainability", "sustainable",
    "recycling", "renewable", "solar", "wind", "biodiversity",
    "conservation", "ecology", "pollution", "net-zero", "netzero",
    "esg", "green", "compost",
})
_ENVIRONMENT_PHRASES = (
    "climate change", "carbon footprint", "sustainability plan",
    "reduce emissions", "renewable energy", "environmental impact",
)

_SPIRITUAL_WORDS = frozenset({
    "spiritual", "spirituality", "meditation", "prayer", "faith",
    "religion", "religious", "sermon", "scripture", "bible", "quran",
    "torah", "mindfulness", "ritual", "church", "mosque", "temple",
    "pastoral", "theology", "devotional",
})
_SPIRITUAL_PHRASES = (
    "spiritual practice", "meditation guide", "sermon outline",
    "interfaith", "faith community", "daily devotion",
)

_SENIOR_WORDS = frozenset({
    "senior", "seniors", "elderly", "aging", "retire", "retirement",
    "medicare", "social-security", "caregiver", "eldercare", "nursing",
    "assisted-living", "memory-care", "grandparent", "grandparents",
})
_SENIOR_PHRASES = (
    "for seniors", "aging parent", "retirement planning", "elder care",
    "assisted living", "caregiver support", "medicare options",
)

_AUTOMOTIVE_WORDS = frozenset({
    "car", "cars", "vehicle", "engine", "transmission", "brake", "brakes",
    "oil", "tire", "tires", "mechanic", "diagnostic", "obd", "ev",
    "electric-vehicle", "charging", "mileage", "mpg", "repair",
})
_AUTOMOTIVE_PHRASES = (
    "car repair", "check engine", "oil change", "brake pads",
    "buy a car", "electric vehicle", "car maintenance",
)

_AGRICULTURE_WORDS = frozenset({
    "farm", "farming", "crop", "crops", "soil", "irrigation", "harvest",
    "livestock", "cattle", "poultry", "greenhouse", "fertilizer",
    "pesticide", "organic", "agronomy", "tractor", "orchard",
})
_AGRICULTURE_PHRASES = (
    "farm plan", "crop rotation", "soil health", "livestock care",
    "greenhouse growing", "agricultural",
)

_MUSIC_WORDS = frozenset({
    "music", "song", "songs", "lyrics", "melody", "chord", "chords",
    "harmony", "tempo", "bpm", "scale", "scales", "riff", "verse",
    "chorus", "bridge", "ableton", "logic", "protools", "midi",
    "synth", "mixing", "mastering", "arrangement", "composer",
})
_MUSIC_PHRASES = (
    "write a song", "chord progression", "music production", "mix this track",
    "song arrangement", "learn guitar", "music theory",
)

_PHOTOGRAPHY_WORDS = frozenset({
    "photography", "photo", "photos", "camera", "lens", "exposure", "aperture",
    "shutter", "iso", "bokeh", "portrait", "landscape", "raw", "lightroom",
    "darkroom", "composition", "framing", "tripod", "flash", "dslr", "mirrorless",
})
_PHOTOGRAPHY_PHRASES = (
    "photo composition", "camera settings", "portrait lighting", "edit photos",
    "photography tips", "shooting in",
)

_FILM_WORDS = frozenset({
    "film", "cinema", "cinematic", "director", "screenplay", "screenwriting",
    "storyboard", "shot-list", "shotlist", "blocking", "editing", "premiere",
    "davinci", "color-grade", "colorgrade", "b-roll", "broll", "documentary",
    "short-film", "feature", "scene", "take", "cast", "crew",
})
_FILM_PHRASES = (
    "short film", "film shot list", "screenplay structure", "color grade",
    "documentary outline", "directing tips",
)

_PODCAST_WORDS = frozenset({
    "podcast", "podcasts", "episode", "episodes", "host", "cohost", "guest",
    "mic", "microphone", "rss", "show-notes", "shownotes", "audiogram",
    "interview", "segment", "sponsor", "patreon",
})
_PODCAST_PHRASES = (
    "podcast episode", "podcast outline", "show notes", "interview questions podcast",
    "start a podcast", "podcast script",
)

_ARCHITECTURE_WORDS = frozenset({
    "architecture", "architect", "building", "blueprint", "floorplan", "floor-plan",
    "facade", "structural", "zoning", "permit", "permits", "site-plan",
    "revit", "bim", "schematic", "massing", "urban",
})
_ARCHITECTURE_PHRASES = (
    "floor plan", "building design", "architectural drawing", "site plan",
    "building code", "architectural concept",
)

_INTERIOR_WORDS = frozenset({
    "interior", "interiors", "furniture", "decor", "decoration", "room",
    "living-room", "bedroom", "kitchen-design", "palette", "moodboard",
    "staging", "renovation", "remodel", "space-planning",
})
_INTERIOR_PHRASES = (
    "interior design", "room layout", "furniture layout", "decorate my",
    "home staging", "mood board",
)

_INSURANCE_WORDS = frozenset({
    "insurance", "premium", "deductible", "claim", "claims", "policy",
    "coverage", "underwriting", "liability", "homeowners", "auto-insurance",
    "life-insurance", "health-insurance", "copay", "coinsurance",
})
_INSURANCE_PHRASES = (
    "file a claim", "insurance policy", "compare insurance", "coverage limits",
    "what does my insurance", "insurance quote",
)

_TAX_WORDS = frozenset({
    "tax", "taxes", "irs", "hmrc", "vat", "gst", "withholding", "deduction",
    "deductions", "write-off", "writeoff", "1040", "w2", "w-2", "1099",
    "filing", "refund", "audit", "estimated-tax", "capital-gains",
})
_TAX_PHRASES = (
    "tax return", "file taxes", "tax deduction", "estimated taxes",
    "tax refund", "self employment tax", "not tax advice",
)

_INVESTING_WORDS = frozenset({
    "invest", "investing", "investment", "portfolio", "stocks", "bonds",
    "etf", "index-fund", "indexfund", "dividend", "brokerage", "roth",
    "401k", "ira", "asset-allocation", "rebalance", "bull", "bear",
})
_INVESTING_PHRASES = (
    "investment portfolio", "stock market", "index funds", "long term investing",
    "asset allocation", "not investment advice",
)

_CRYPTO_WORDS = frozenset({
    "crypto", "cryptocurrency", "bitcoin", "ethereum", "blockchain", "defi",
    "nft", "wallet", "metamask", "token", "tokens", "web3", "staking",
    "airdrop", "gas", "smart-contract", "smartcontract", "solana",
})
_CRYPTO_PHRASES = (
    "crypto wallet", "smart contract", "defi protocol", "bitcoin",
    "not financial advice crypto",
)

_RETAIL_WORDS = frozenset({
    "retail", "store", "stores", "merchandising", "inventory", "sku",
    "pos", "checkout", "shelf", "brick-and-mortar", "shop", "shopping",
    "customer", "foot-traffic", "loss-prevention",
})
_RETAIL_PHRASES = (
    "retail store", "inventory management", "store layout", "retail ops",
    "point of sale",
)

_ECOMMERCE_WORDS = frozenset({
    "ecommerce", "e-commerce", "shopify", "woocommerce", "cart", "checkout",
    "conversion", "product-page", "listing", "amazon", "etsy", "fulfillment",
    "dropship", "dropshipping", "sku", "returns",
})
_ECOMMERCE_PHRASES = (
    "online store", "product listing", "shopify store", "ecommerce funnel",
    "abandoned cart", "amazon listing",
)

_LOGISTICS_WORDS = frozenset({
    "logistics", "shipping", "freight", "warehouse", "fulfillment", "supply-chain",
    "supplychain", "inventory", "carrier", "last-mile", "lastmile", "customs",
    "import", "export", "3pl", "routing", "delivery",
})
_LOGISTICS_PHRASES = (
    "supply chain", "shipping quote", "warehouse ops", "last mile delivery",
    "freight shipping",
)

_MANUFACTURING_WORDS = frozenset({
    "manufacturing", "factory", "production", "assembly", "bom", "cnc",
    "lean", "six-sigma", "sixsigma", "throughput", "yield", "scrap",
    "tooling", "mold", "injection", "quality-control",
})
_MANUFACTURING_PHRASES = (
    "manufacturing process", "bill of materials", "production line",
    "lean manufacturing", "factory floor",
)

_CONSTRUCTION_WORDS = frozenset({
    "construction", "contractor", "subcontractor", "blueprint", "framing",
    "foundation", "concrete", "roofing", "electrical", "plumbing",
    "permit", "permits", "jobsite", "site", "bid", "estimate",
})
_CONSTRUCTION_PHRASES = (
    "construction project", "building permit", "contractor bid",
    "home construction", "jobsite safety",
)

_ROBOTICS_WORDS = frozenset({
    "robot", "robots", "robotics", "actuator", "servo", "ros", "slam",
    "kinematics", "end-effector", "endeffector", "autonomous", "drone",
    "uav", "path-planning", "pathplanning",
})
_ROBOTICS_PHRASES = (
    "robot arm", "robotics project", "ros package", "autonomous robot",
    "drone navigation",
)

_MATH_WORDS = frozenset({
    "algebra", "calculus", "geometry", "trigonometry", "proof", "theorem",
    "derivative", "integral", "matrix", "vector", "equation", "equations",
    "probability", "combinatorics", "number-theory", "linear-algebra",
})
_MATH_PHRASES = (
    "math problem", "solve this equation", "prove that", "calculus help",
    "linear algebra", "geometry proof",
)

_PHILOSOPHY_WORDS = frozenset({
    "philosophy", "philosophical", "ethics", "ethical", "metaphysics",
    "epistemology", "ontology", "logic", "argument", "fallacy",
    "kant", "nietzsche", "stoicism", "existentialism", "utilitarianism",
})
_PHILOSOPHY_PHRASES = (
    "philosophical argument", "ethical dilemma", "what is consciousness",
    "thought experiment", "philosophy essay",
)

_PETS_WORDS = frozenset({
    "dog", "dogs", "cat", "cats", "pet", "pets", "puppy", "kitten",
    "vet", "veterinary", "leash", "crate", "litter", "grooming",
    "training", "bark", "meow", "aquarium", "fish", "bird", "parrot",
})
_PETS_PHRASES = (
    "dog training", "cat behavior", "pet care", "vet visit",
    "new puppy", "not a vet",
)

_CHILDCARE_WORDS = frozenset({
    "daycare", "nanny", "babysitter", "preschool", "toddler", "infant",
    "diaper", "nap", "milestones", "potty", "childcare", "babysitting",
})
_CHILDCARE_PHRASES = (
    "childcare routine", "toddler schedule", "daycare tips",
    "babysitting guide", "infant sleep",
)

_IMMIGRATION_WORDS = frozenset({
    "visa", "visas", "immigration", "immigrant", "green-card", "greencard",
    "passport", "asylum", "citizenship", "naturalization", "i-20", "i20",
    "h1b", "h-1b", "f1", "f-1", "opt", "uscis", "border", "consulate",
})
_IMMIGRATION_PHRASES = (
    "visa application", "immigration process", "green card", "student visa",
    "work visa", "not immigration legal advice",
)

_THERAPY_WORDS = frozenset({
    "therapy", "therapist", "counseling", "counsellor", "anxiety", "depression",
    "grief", "trauma", "cbt", "dbt", "mindfulness", "boundaries", "self-care",
    "selfcare", "burnout", "mental-health", "mentalhealth",
})
_THERAPY_PHRASES = (
    "mental health", "coping skills", "feel anxious", "therapy homework",
    "not a therapist", "emotional support",
)

_LIBRARY_WORDS = frozenset({
    "library", "librarian", "catalog", "catalogue", "archive", "archives",
    "dewey", "oclc", "reference", "interlibrary", "collection", "stacks",
})
_LIBRARY_PHRASES = (
    "library research", "find sources", "library catalog", "archival research",
)

_THEATER_WORDS = frozenset({
    "theater", "theatre", "play", "plays", "stage", "rehearsal", "monologue",
    "audition", "blocking", "script", "cast", "director", "broadway",
    "improv", "improvisation",
})
_THEATER_PHRASES = (
    "stage play", "audition monologue", "theater production", "rehearsal plan",
)

_DANCE_WORDS = frozenset({
    "dance", "dancing", "ballet", "choreography", "choreographer", "routine",
    "hip-hop", "hiphop", "contemporary", "jazz", "rehearsal", "studio",
})
_DANCE_PHRASES = (
    "dance routine", "choreography ideas", "ballet class", "dance practice",
)

_WEATHER_WORDS = frozenset({
    "weather", "forecast", "rain", "storm", "hurricane", "tornado",
    "temperature", "humidity", "climate", "snow", "wind", "radar",
    "meteorology",
})
_WEATHER_PHRASES = (
    "weather forecast", "storm prep", "will it rain", "severe weather",
)

_ASTRONOMY_WORDS = frozenset({
    "astronomy", "astronomical", "telescope", "planet", "planets", "star",
    "stars", "galaxy", "nebula", "constellation", "orbit", "eclipse",
    "nasa", "cosmo", "astrophysics", "moon", "mars",
})
_ASTRONOMY_PHRASES = (
    "night sky", "telescope setup", "planet watching", "astronomy basics",
)

_COMPLIANCE_WORDS = frozenset({
    "compliance", "regulatory", "audit", "soc2", "soc 2", "iso27001",
    "hipaa", "pci", "gdpr", "policy", "control", "controls", "attestation",
    "risk-assessment", "riskassessment",
})
_COMPLIANCE_PHRASES = (
    "compliance checklist", "soc 2", "regulatory compliance", "audit prep",
    "internal controls",
)

_OPERATIONS_WORDS = frozenset({
    "operations", "ops", "sops", "sop", "runbook", "playbook", "process",
    "workflow", "capacity", "sla", "on-call", "oncall", "incident",
    "postmortem", "post-mortem", "reliability",
})
_OPERATIONS_PHRASES = (
    "ops runbook", "standard operating procedure", "operations plan",
    "on call rotation", "incident postmortem",
)

_PROCUREMENT_WORDS = frozenset({
    "procurement", "purchasing", "vendor", "vendors", "rfp", "rfq", "po",
    "purchase-order", "purchaseorder", "supplier", "sourcing", "contracting",
})
_PROCUREMENT_PHRASES = (
    "request for proposal", "vendor selection", "purchase order",
    "supplier negotiation", "procurement process",
)

_QUALITY_WORDS = frozenset({
    "qa", "quality", "testing", "test-plan", "testplan", "bug", "bugs",
    "regression", "qa-engineer", "qc", "inspection", "acceptance-test",
    "uat", "defect", "defects",
})
_QUALITY_PHRASES = (
    "test plan", "quality assurance", "regression testing", "bug report",
    "qa checklist", "acceptance testing",
)

_GROWTH_WORDS = frozenset({
    "growth", "activation", "retention", "referral", "virality", "k-factor",
    "kfactor", "north-star", "experiment", "experiments", "funnel", "aha-moment",
    "onboarding", "cohort",
})
_GROWTH_PHRASES = (
    "growth experiment", "activation metric", "retention curve",
    "growth loops", "product-led growth",
)

_UX_RESEARCH_WORDS = frozenset({
    "usability", "user-research", "userresearch", "interview", "interviews",
    "survey", "surveys", "persona", "journey", "affinity", "synthesis",
    "diary-study", "card-sort", "cardsort", "prototype-test",
})
_UX_RESEARCH_PHRASES = (
    "user research", "usability test", "user interview", "research synthesis",
    "journey map", "affinity map",
)

_STATS_WORDS = frozenset({
    "statistics", "statistical", "p-value", "pvalue", "confidence-interval",
    "hypothesis-test", "anova", "regression", "bayesian", "sampling",
    "distribution", "variance", "stddev", "correlation",
})
_STATS_PHRASES = (
    "statistical significance", "hypothesis test", "confidence interval",
    "interpret these stats", "stats help",
)

_GENEALOGY_WORDS = frozenset({
    "genealogy", "ancestry", "family-tree", "familytree", "forebear",
    "census", "records", "lineage", "dna", "heritage",
})
_GENEALOGY_PHRASES = (
    "family tree", "genealogy research", "ancestry records", "dna relatives",
)

_COLLECTING_WORDS = frozenset({
    "collect", "collecting", "collection", "collector", "rare", "vintage",
    "antique", "grading", "auction", "ebay", "catalog", "catalogue",
})
_COLLECTING_PHRASES = (
    "start a collection", "value of this", "collector tips", "rare find",
)

_OUTDOORS_WORDS = frozenset({
    "hike", "hiking", "camping", "backpack", "trail", "outdoors", "wilderness",
    "climb", "climbing", "kayak", "fishing", "hunt", "hunting", "gear",
})
_OUTDOORS_PHRASES = (
    "hiking trail", "camping trip", "packing list hike", "outdoor gear",
    "backpacking plan",
)

_GARDENING_WORDS = frozenset({
    "garden", "gardening", "plant", "plants", "soil", "seed", "seeds",
    "compost", "pruning", "mulch", "fertilizer", "vegetable", "flower",
    "lawn", "weeds", "greenhouse",
})
_GARDENING_PHRASES = (
    "vegetable garden", "plant care", "when to plant", "garden plan",
    "lawn care",
)

_BAKING_WORDS = frozenset({
    "bake", "baking", "bread", "sourdough", "cake", "cookie", "cookies",
    "pastry", "dough", "yeast", "frosting", "oven", "bakeware",
})
_BAKING_PHRASES = (
    "bake bread", "cake recipe", "sourdough starter", "cookie recipe",
    "pastry tips",
)

_COFFEE_WORDS = frozenset({
    "coffee", "espresso", "latte", "pour-over", "pourover", "grinder",
    "roast", "roasting", "barista", "brew", "aeropress", "chemex", "v60",
})
_COFFEE_PHRASES = (
    "brew coffee", "espresso recipe", "coffee beans", "latte art",
    "coffee tasting",
)

_WINE_WORDS = frozenset({
    "wine", "wines", "vineyard", "sommelier", "tasting", "pairing",
    "cabernet", "chardonnay", "pinot", "vintage", "cellar", "decant",
})
_WINE_PHRASES = (
    "wine pairing", "wine tasting", "choose a wine", "wine cellar",
)

_BEER_WORDS = frozenset({
    "beer", "beers", "brewery", "ipa", "lager", "stout", "homebrew",
    "homebrewing", "ferment", "hops", "malt", "keg",
})
_BEER_PHRASES = (
    "homebrew recipe", "beer tasting", "brew beer", "ipa recipe",
)

# Explicit “I am a …” self-labels (rare but strong).

_AVIATION_WORDS = frozenset({
    "pilot", "aircraft", "airplane", "airport", "runway", "atc", "flight", "airbus", "boeing", "jet", "turbine", "hangar", "avionics",
    "cessna", "ifr", "vfr", "ils", "vor", "notam", "altimeter", "airspeed", "cockpit", "flaps", "aileron", "taxiway",
})
_AVIATION_PHRASES = (
    'flight plan', 'pilot training', 'airport operations', 'aircraft maintenance',
    'ifr approach', 'vfr flight', 'instrument approach', 'how do i fly',
)


_MARITIME_WORDS = frozenset({
    "ship", "boat", "vessel", "marina", "harbor", "harbour", "nautical", "sailing", "yacht", "cargo", "port", "dock", "crew",
})
_MARITIME_PHRASES = (
    'ship operations', 'boat maintenance', 'sailing plan', 'port logistics',
)


_ENERGY_WORDS = frozenset({
    "energy", "grid", "electricity", "power", "solar", "wind", "turbine", "nuclear", "utility", "kilowatt", "megawatt", "storage", "battery",
})
_ENERGY_PHRASES = (
    'power grid', 'renewable energy', 'energy storage', 'utility bill',
)


_TELECOM_WORDS = frozenset({
    "telecom", "network", "cellular", "5g", "lte", "fiber", "broadband", "isp", "router", "modem", "spectrum", "latency",
})
_TELECOM_PHRASES = (
    'network outage', 'fiber internet', 'cellular coverage', 'telecom plan',
)


_MEDIA_WORDS = frozenset({
    "broadcast", "tv", "radio", "streaming", "platform", "publisher", "imprint", "ratings", "audience", "channel",
})
_MEDIA_PHRASES = (
    'media strategy', 'broadcast schedule', 'content platform', 'publishing pipeline',
)


_PR_WORDS = frozenset({
    "pr", "publicity", "reputation", "crisis", "spokesperson", "presskit", "press-kit", "messaging", "narrative",
})
_PR_PHRASES = (
    'press strategy', 'crisis communication', 'public relations', 'media relations',
)


_SOCIAL_WORK_WORDS = frozenset({
    "casework", "caseload", "client", "welfare", "foster", "shelter", "housing-insecure", "social-worker",
})
_SOCIAL_WORK_PHRASES = (
    'social work', 'case management', 'client support', 'community resources',
)


_ACCOUNTING_WORDS = frozenset({
    "ledger", "journal", "entry", "debit", "credit", "accrual", "bookkeeping", "accountant", "cpa", "reconcile",
})
_ACCOUNTING_PHRASES = (
    'general ledger', 'journal entry', 'reconcile accounts', 'chart of accounts',
)


_ACTING_WORDS = frozenset({
    "actor", "acting", "monologue", "scene", "character", "motivation", "improvisation", "cold-read", "coldread",
})
_ACTING_PHRASES = (
    'acting monologue', 'character work', 'scene study', 'audition prep acting',
)


_COMEDY_WORDS = frozenset({
    "comedy", "joke", "jokes", "standup", "stand-up", "sketch", "bit", "punchline", "improv", "roast",
})
_COMEDY_PHRASES = (
    'standup set', 'comedy sketch', 'write jokes', 'punch up this',
)


_WOODWORKING_WORDS = frozenset({
    "woodworking", "saw", "chisel", "plane", "jointer", "tablesaw", "router", "joinery", "lumber", "timber", "dovetail",
})
_WOODWORKING_PHRASES = (
    'woodworking project', 'build a table', 'dovetail joint', 'shop safety',
)


_METALWORKING_WORDS = frozenset({
    "welding", "welder", "mill", "lathe", "forge", "fabrication", "steel", "aluminum", "mig", "tig", "grinder",
})
_METALWORKING_PHRASES = (
    'welding project', 'metal fabrication', 'use a lathe', 'mig weld',
)


_ELECTRONICS_WORDS = frozenset({
    "electronics", "circuit", "resistor", "capacitor", "arduino", "raspberry", "oscilloscope", "soldering", "pcb",
})
_ELECTRONICS_PHRASES = (
    'electronics project', 'solder this', 'arduino circuit', 'breadboard',
)


_PRINTING_3D_WORDS = frozenset({
    "3d-print", "3dprinting", "filament", "pla", "abs", "resin", "slicer", "ender", "prusa", "nozzle", "bed-level",
})
_PRINTING_3D_PHRASES = (
    '3d print', 'slicer settings', 'print failed', 'bed leveling',
)


_SEWING_WORDS = frozenset({
    "sewing", "seam", "stitch", "hem", "zipper", "fabric", "pattern", "serger", "bobbin", "tailor",
})
_SEWING_PHRASES = (
    'sew a', 'sewing pattern', 'hem pants', 'install a zipper',
)


_KNITTING_WORDS = frozenset({
    "knit", "knitting", "yarn", "purl", "gauge", "cast-on", "caston", "frogging", "skein", "needles",
})
_KNITTING_PHRASES = (
    'knitting pattern', 'cast on', 'yarn weight', 'fix my knitting',
)


_CHESS_WORDS = frozenset({
    "chess", "opening", "endgame", "gambit", "pawn", "knight", "bishop", "rook", "queen", "checkmate", "elo",
})
_CHESS_PHRASES = (
    'chess opening', 'analyze this game', 'tactics puzzle', 'endgame technique',
)


_TABLETOP_WORDS = frozenset({
    "dnd", "d&d", "pathfinder", "ttrpg", "campaign", "dm", "gm", "dice", "character-sheet", "boardgame", "catan",
})
_TABLETOP_PHRASES = (
    'd&d campaign', 'dungeon master', 'board game night', 'character sheet',
)


_ANIME_WORDS = frozenset({
    "anime", "manga", "otaku", "shonen", "shojo", "isekai", "studio-ghibli", "waifu",
})
_ANIME_PHRASES = (
    'anime recommendation', 'manga series', 'best anime', 'spoiler free',
)


_COMICS_WORDS = frozenset({
    "comic", "comics", "graphic-novel", "panel", "panels", "inking", "lettering", "sequential",
})
_COMICS_PHRASES = (
    'comic script', 'graphic novel', 'panel layout', 'comic page',
)


_SCUBA_WORDS = frozenset({
    "scuba", "dive", "diving", "regulator", "buoyancy", "nitrox", "padi", "ssi", "wreck", "reef",
})
_SCUBA_PHRASES = (
    'scuba dive plan', 'buoyancy control', 'dive computer', 'open water',
)


_CYCLING_WORDS = frozenset({
    "bike", "bicycle", "cycling", "cadence", "peloton", "roadbike", "mtb", "gears", "drivetrain",
})
_CYCLING_PHRASES = (
    'bike fit', 'cycling training', 'fix my bike', 'road cycling',
)


_RUNNING_WORDS = frozenset({
    "running", "runner", "marathon", "half-marathon", "5k", "10k", "pace", "splits", "stride",
})
_RUNNING_PHRASES = (
    'running plan', 'marathon training', 'improve pace', 'race day',
)


_MARTIAL_ARTS_WORDS = frozenset({
    "karate", "judo", "bjj", "jiujitsu", "taekwondo", "boxing", "muay-thai", "sparring", "belt", "dojo", "kata",
})
_MARTIAL_ARTS_PHRASES = (
    'martial arts', 'sparring tips', 'belt test', 'jiu jitsu',
)


_NUTRITION_WORDS = frozenset({
    "nutrition", "macros", "calories", "protein", "carbs", "fiber", "meal-prep", "dietitian", "macros",
})
_NUTRITION_PHRASES = (
    'meal prep macros', 'nutrition plan', 'calorie target', 'balanced diet',
)


_PRODUCTIVITY_WORDS = frozenset({
    "productivity", "focus", "deep-work", "pomodoro", "gtd", "prioritization", "timeblock", "habits",
})
_PRODUCTIVITY_PHRASES = (
    'productivity system', 'deep work', 'time blocking', 'focus better',
)


_PKM_WORDS = frozenset({
    "pkm", "zettelkasten", "obsidian", "logseq", "roam", "second-brain", "notes", "evergreen",
})
_PKM_PHRASES = (
    'obsidian vault', 'zettelkasten', 'second brain', 'note system',
)


_DEVOPS_WORDS = frozenset({
    "devops", "cicd", "pipeline", "terraform", "ansible", "helm", "jenkins", "github-actions", "deploy",
})
_DEVOPS_PHRASES = (
    'ci cd pipeline', 'terraform module', 'deploy pipeline', 'infra as code',
)


_CLOUD_WORDS = frozenset({
    "aws", "azure", "gcp", "cloud", "s3", "ec2", "kubernetes", "lambda", "kubernetesformation", "cloudformation",
})
_CLOUD_PHRASES = (
    'cloud architecture', 'aws setup', 'gcp project', 'azure resource',
)


_NETWORKING_WORDS = frozenset({
    "networking", "subnet", "vlan", "dns", "dhcp", "tcp", "udp", "firewall", "router", "switch", "wifi",
})
_NETWORKING_PHRASES = (
    'network troubleshooting', 'subnet design', 'dns issue', 'wifi dead',
)


_DATABASE_WORDS = frozenset({
    "database", "postgres", "mysql", "sqlite", "schema", "index", "query-plan", "normalization", "orm",
})
_DATABASE_PHRASES = (
    'database schema', 'slow query', 'postgres index', 'migrate database',
)


_MOBILE_WORDS = frozenset({
    "ios", "android", "mobile", "swift", "kotlin", "flutter", "react-native", "app-store", "play-store",
})
_MOBILE_PHRASES = (
    'ios app', 'android app', 'mobile ux', 'app store release',
)


_WEBDEV_WORDS = frozenset({
    "html", "css", "react", "nextjs", "vue", "svelte", "backend", "frontend", "api", "rest", "graphql", "webpack",
})
_WEBDEV_PHRASES = (
    'web app', 'frontend bug', 'backend api', 'responsive layout',
)


_EMBEDDED_WORDS = frozenset({
    "embedded", "firmware", "mcu", "microcontroller", "bare-metal", "freertos", "stm32", "esp32",
})
_EMBEDDED_PHRASES = (
    'embedded firmware', 'stm32', 'esp32 project', 'bare metal',
)


_IOT_WORDS = frozenset({
    "iot", "sensor", "sensors", "mqtt", "zigbee", "z-wave", "home-assistant", "smart-home", "device", "fleet",
})
_IOT_PHRASES = (
    'iot device', 'mqtt broker', 'smart home', 'sensor network',
)


_ARVR_WORDS = frozenset({
    "ar", "vr", "xr", "headset", "meta-quest", "hololens", "unity", "unreal", "spatial",
})
_ARVR_PHRASES = (
    'vr experience', 'ar app', 'quest development', 'spatial ui',
)


_FREELANCE_WORDS = frozenset({
    "freelance", "freelancing", "client", "retainer", "invoice", "proposal", "rate", "rates",
})
_FREELANCE_PHRASES = (
    'freelance rate', 'client proposal', 'freelance contract', 'find clients',
)


_CONSULTING_WORDS = frozenset({
    "consulting", "consultant", "engagement", "deliverable", "recommendation", "framework",
})
_CONSULTING_PHRASES = (
    'consulting engagement', 'client deliverable', 'consulting deck',
)


_COACHING_WORDS = frozenset({
    "coaching", "coach", "mentor", "mentorship", "accountability", "goals", "habit",
})
_COACHING_PHRASES = (
    'life coaching', 'coaching session', 'accountability plan', 'mentorship',
)


_SPEAKING_WORDS = frozenset({
    "keynote", "speech", "talk", "toastmasters", "presentation", "slides", "stage", "presence",
})
_SPEAKING_PHRASES = (
    'keynote talk', 'public speaking', 'conference talk', 'speech outline',
)


_RELATIONSHIPS_WORDS = frozenset({
    "relationship", "relationships", "partner", "spouse", "marriage", "communication", "boundaries",
})
_RELATIONSHIPS_PHRASES = (
    'relationship advice', 'talk to my partner', 'healthy boundaries',
)


_DATING_WORDS = frozenset({
    "dating", "profile", "tinder", "hinge", "bumble", "first-date", "date",
})
_DATING_PHRASES = (
    'dating profile', 'first date ideas', 'online dating',
)


_HISTORY_WORDS = frozenset({
    "history", "historical", "century", "empire", "war", "revolution", "primary-source", "historiography",
})
_HISTORY_PHRASES = (
    'historical context', 'history essay', 'primary source', 'what happened in',
)


_GEOGRAPHY_WORDS = frozenset({
    "geography", "map", "maps", "region", "continent", "climate-zone", "cartography", "gis",
})
_GEOGRAPHY_PHRASES = (
    'geography of', 'map this', 'regional geography',
)


_CHEMISTRY_WORDS = frozenset({
    "chemistry", "molecule", "reaction", "stoichiometry", "acid", "base", "organic", "inorganic", "lab",
})
_CHEMISTRY_PHRASES = (
    'balance reaction', 'organic chemistry', 'chemistry homework', 'molarity',
)


_BIOLOGY_WORDS = frozenset({
    "biology", "cell", "dna", "gene", "evolution", "ecology", "anatomy", "physiology", "microbiology",
})
_BIOLOGY_PHRASES = (
    'cell biology', 'dna transcription', 'ecology of', 'biology homework',
)


_PHYSICS_WORDS = frozenset({
    "physics", "force", "energy", "momentum", "quantum", "relativity", "kinematics", "newton",
})
_PHYSICS_PHRASES = (
    'physics problem', 'newton laws', 'kinematic equation', 'quantum basics',
)


_MEDICINE_WORDS = frozenset({
    "medicine", "medical", "clinical", "diagnosis", "differential", "pathophysiology",
})
_MEDICINE_PHRASES = (
    'medical literacy', 'clinical overview', 'not a doctor',
)


_NURSING_WORDS = frozenset({
    "nursing", "nurse", "rn", "lpn", "care-plan", "vitals", "charting", "bedside",
})
_NURSING_PHRASES = (
    'nursing care plan', 'nursing school', 'vitals chart',
)


_PHARMACY_WORDS = frozenset({
    "pharmacy", "pharmacist", "prescription", "dosage", "drug", "interaction", "otc",
})
_PHARMACY_PHRASES = (
    'drug interaction', 'pharmacy school', 'medication counseling',
)


_DENTAL_WORDS = frozenset({
    "dental", "dentist", "tooth", "cavity", "braces", "orthodontics", "oral", "hygiene",
})
_DENTAL_PHRASES = (
    'dental care', 'tooth pain', 'braces questions',
)


_VETERINARY_WORDS = frozenset({
    "veterinary", "vet", "animal", "clinic", "diagnosis", "pet-health",
})
_VETERINARY_PHRASES = (
    'vet advice', 'animal health', 'not a vet',
)


_MILITARY_WORDS = frozenset({
    "military", "army", "navy", "airforce", "marines", "doctrine", "ops", "logistics", "rank",
})
_MILITARY_PHRASES = (
    'military career', 'doctrine overview', 'service branch',
)


_FIRE_WORDS = frozenset({
    "fire", "firefighter", "smoke", "extinguisher", "evacuation", "arson", "sprinkler",
})
_FIRE_PHRASES = (
    'fire safety', 'evacuate plan', 'use extinguisher',
)


_POLICE_WORDS = frozenset({
    "police", "officer", "law-enforcement", "911", "dispatch", "precinct",
})
_POLICE_PHRASES = (
    'police report', 'public safety', 'law enforcement process',
)


_GEOLOGY_WORDS = frozenset({
    "geology", "rock", "mineral", "tectonic", "earthquake", "volcano", "sediment",
})
_GEOLOGY_PHRASES = (
    'rock identification', 'tectonic plates', 'geology basics',
)


_OCEAN_WORDS = frozenset({
    "ocean", "marine", "sea", "tide", "current", "coral", "reef", "fisheries", "oceanography",
})
_OCEAN_PHRASES = (
    'ocean currents', 'marine biology', 'tide chart',
)


_ARCHAEOLOGY_WORDS = frozenset({
    "archaeology", "archaeological", "excavation", "artifact", "stratigraphy", "dig", "site",
})
_ARCHAEOLOGY_PHRASES = (
    'archaeological dig', 'artifact context', 'field methods',
)


_LINGUISTICS_WORDS = frozenset({
    "linguistics", "phonology", "morphology", "syntax", "semantics", "pragmatics", "sociolinguistics",
})
_LINGUISTICS_PHRASES = (
    'linguistic analysis', 'phonology of', 'syntax tree',
)

_FITNESS_WORDS = frozenset({
    "gym", "workout", "workouts", "hypertrophy", "strength", "reps", "sets", "dumbbell", "barbell", "squat", "deadlift", "bench",
})
_FITNESS_PHRASES = (
    'gym program', 'workout plan', 'build muscle', 'strength training', 'leg day',
)

_YOGA_WORDS = frozenset({
    "yoga", "asana", "vinyasa", "yin", "pranayama", "downward-dog", "savasana", "namaste", "yogic",
})
_YOGA_PHRASES = (
    'yoga sequence', 'yoga class', 'vinyasa flow', 'yoga pose',
)

_CLIMBING_WORDS = frozenset({
    "climbing", "bouldering", "climber", "crag", "belay", "lead-climbing", "toprope", "beta", "crimp", "dyno", "send",
})
_CLIMBING_PHRASES = (
    'climbing beta', 'bouldering problem', 'belay technique', 'send this route',
)

_GOLF_WORDS = frozenset({
    "golf", "golfer", "driver", "iron", "putt", "putting", "handicap", "fairway", "green", "wedge", "par", "birdie",
})
_GOLF_PHRASES = (
    'golf swing', 'golf lesson', 'lower handicap', 'putting drill',
)

_FISHING_WORDS = frozenset({
    "fishing", "fisherman", "angler", "lure", "bait", "rod", "reel", "cast", "fly-fishing", "bass", "trout", "catch",
})
_FISHING_PHRASES = (
    'fishing trip', 'fly fishing', 'best lure', 'how to fish',
)

_SWIMMING_WORDS = frozenset({
    "swimming", "swim", "freestyle", "backstroke", "breaststroke", "butterfly", "lap", "laps", "pool", "open-water",
})
_SWIMMING_PHRASES = (
    'swim workout', 'freestyle technique', 'swim set', 'open water swim',
)

_SKIING_WORDS = frozenset({
    "skiing", "ski", "skier", "snowboard", "snowboarding", "piste", "powder", "carving", "bindings", "slopes",
})
_SKIING_PHRASES = (
    'ski technique', 'snowboard tips', 'powder day', 'ski lesson',
)

_MOTORCYCLE_WORDS = frozenset({
    "motorcycle", "motorbike", "bike-ride", "helmet", "leathers", "cc", "throttle", "clutch", "harley", "sportbike",
})
_MOTORCYCLE_PHRASES = (
    'motorcycle maintenance', 'learn to ride', 'motorcycle gear', 'bike setup',
)

_DRONE_WORDS = frozenset({
    "drone", "drones", "uas", "uav", "quadcopter", "fpv", "gimbal", "dj i", "dji", "airspace",
})
_DRONE_PHRASES = (
    'drone flight', 'fpv setup', 'drone photography', 'uas rules',
)

_GAME_DEV_WORDS = frozenset({
    "gamedev", "game-dev", "unity", "unreal", "godot", "gameplay", "level-design", "mechanics", "sprite", "prefab", "shader",
})
_GAME_DEV_PHRASES = (
    'game design doc', 'unity tutorial', 'godot project', 'level design', 'game loop',
)

_ANIMATION_WORDS = frozenset({
    "animation", "animate", "animator", "keyframes", "rigging", "tween", "motion-graphics", "after-effects", "blender-anim",
})
_ANIMATION_PHRASES = (
    'animation principles', 'keyframe animation', 'rig a character', 'motion graphics',
)

_POETRY_WORDS = frozenset({
    "poetry", "poem", "poems", "verse", "sonnet", "haiku", "stanza", "meter", "rhyme", "free-verse",
})
_POETRY_PHRASES = (
    'write a poem', 'poetry workshop', 'revise this poem', 'haiku about',
)

_MAKEUP_WORDS = frozenset({
    "makeup", "foundation", "concealer", "eyeshadow", "lipstick", "contour", "primer", "mascara", "blush", "highlighter",
})
_MAKEUP_PHRASES = (
    'makeup tutorial', 'everyday makeup', 'bridal makeup', 'eye makeup',
)

_HAIR_WORDS = frozenset({
    "hair", "hairstyle", "haircut", "salon", "shampoo", "conditioner", "blowout", "curl", "braid", "colorist",
})
_HAIR_PHRASES = (
    'hair care', 'haircut ideas', 'style my hair', 'hair color',
)

_SKINCARE_WORDS = frozenset({
    "skincare", "serum", "retinol", "moisturizer", "cleanser", "spf", "sunscreen", "acne", "routine", "niacinamide",
})
_SKINCARE_PHRASES = (
    'skincare routine', 'retinol how to', 'acne care', 'morning skincare',
)

_WEDDING_WORDS = frozenset({
    "wedding", "bride", "groom", "ceremony", "reception", "venue", "vows", "bridal", "honeymoon", "rsvp",
})
_WEDDING_PHRASES = (
    'wedding plan', 'wedding budget', 'wedding timeline', 'write vows',
)

_PREGNANCY_WORDS = frozenset({
    "pregnancy", "pregnant", "prenatal", "trimester", "ultrasound", "nursery", "birth-plan", "maternity",
})
_PREGNANCY_PHRASES = (
    'pregnancy tips', 'birth plan', 'prenatal checklist', 'third trimester',
)

_SLEEP_WORDS = frozenset({
    "sleep", "insomnia", "circadian", "melatonin", "naps", "bedtime", "sleep-hygiene", "night-owls",
})
_SLEEP_PHRASES = (
    'sleep better', 'sleep schedule', 'insomnia tips', 'sleep hygiene',
)

_FIRST_AID_WORDS = frozenset({
    "first-aid", "cpr", "aed", "bandage", "hemorrhage", "choking", "heimlich", "triage", "emergency",
})
_FIRST_AID_PHRASES = (
    'first aid kit', 'how to cpr', 'stop bleeding', 'choking response',
)

_PUBLIC_HEALTH_WORDS = frozenset({
    # No bare "who" — matches English “who is …”.
    "epidemiology", "outbreak", "vaccination", "herd-immunity", "surveillance",
    "health-equity", "cdc", "pandemic",
})
_PUBLIC_HEALTH_PHRASES = (
    'public health', 'outbreak response', 'health equity', 'epidemiology basics',
    'world health organization', 'who guidelines', 'who pandemic',
)

_ML_AI_WORDS = frozenset({
    "machine-learning", "ml", "llm", "transformer", "embedding", "fine-tune", "finetune", "inference", "dataset", "pytorch", "tensorflow", "sklearn", "rag", "prompt-engineering",
})
_ML_AI_PHRASES = (
    'train a model', 'fine tune llm', 'rag pipeline', 'ml experiment', 'model evaluation',
)

_SRE_WORDS = frozenset({
    "sre", "slo", "sla", "error-budget", "toil", "oncall", "on-call", "incident", "postmortem", "reliability",
})
_SRE_PHRASES = (
    'error budget', 'slo definition', 'incident response', 'on-call rotation', 'postmortem',
)

_SYSTEM_DESIGN_WORDS = frozenset({
    "system-design", "scalability", "throughput", "latency", "sharding", "replication", "load-balancer", "cap-theorem", "consistency",
})
_SYSTEM_DESIGN_PHRASES = (
    'system design interview', 'design twitter', 'scale this service', 'architecture tradeoffs',
)

_TECH_WRITING_WORDS = frozenset({
    "documentation", "docs", "readme", "api-docs", "changelog", "runbook", "howto", "tutorial-write",
})
_TECH_WRITING_PHRASES = (
    'write documentation', 'api docs', 'improve readme', 'technical writing', 'user guide',
)

_PROJECT_MGMT_WORDS = frozenset({
    "project-management", "gantt", "milestone", "wbs", "stakeholder", "raid-log", "timeline", "deliverables", "pmp",
})
_PROJECT_MGMT_PHRASES = (
    'project plan', 'project timeline', 'risk register', 'stakeholder update',
)

_AGILE_WORDS = frozenset({
    "agile", "scrum", "kanban", "sprint", "backlog", "standup", "retrospective", "velocity", "story-points", "burndown",
})
_AGILE_PHRASES = (
    'sprint planning', 'scrum ceremony', 'kanban board', 'retro agenda', 'story points',
)

_REMOTE_WORK_WORDS = frozenset({
    "remote", "wfh", "hybrid", "async", "timezone", "zoom-fatigue", "home-office", "distributed-team",
})
_REMOTE_WORK_PHRASES = (
    'remote work', 'work from home', 'async standup', 'hybrid schedule',
)

_CONTENT_CREATOR_WORDS = frozenset({
    "youtube", "tiktok", "instagram", "newsletter", "substack", "creator", "thumbnail", "monetize", "subscribers", "shorts",
})
_CONTENT_CREATOR_PHRASES = (
    'youtube script', 'tiktok idea', 'content calendar', 'grow my channel', 'newsletter issue',
)

_SEO_WORDS = frozenset({
    "seo", "serp", "backlink", "backlinks", "keyword", "keywords", "meta-description", "sitemap", "crawl", "indexing",
})
_SEO_PHRASES = (
    'seo audit', 'keyword research', 'rank for', 'on-page seo', 'technical seo',
)

_BRAND_WORDS = frozenset({
    "brand", "branding", "brand-voice", "identity", "logo-system", "positioning", "messaging", "brandbook",
})
_BRAND_PHRASES = (
    'brand strategy', 'brand voice', 'brand guidelines', 'positioning statement',
)

_NEGOTIATION_WORDS = frozenset({
    "negotiate", "negotiation", "batna", "counteroffer", "deal", "leverage", "concession", "anchor",
})
_NEGOTIATION_PHRASES = (
    'salary negotiation', 'negotiate offer', 'deal negotiation', 'counter offer',
)

_PATENT_WORDS = frozenset({
    "patent", "patents", "trademark", "copyright", "prior-art", "provisional", "uspto", "ip", "intellectual-property",
})
_PATENT_PHRASES = (
    'file a patent', 'prior art search', 'trademark application', 'patent claim',
)

_HOMESCHOOL_WORDS = frozenset({
    "homeschool", "homeschooling", "unschooling", "curriculum", "co-op", "lesson-at-home",
})
_HOMESCHOOL_PHRASES = (
    'homeschool curriculum', 'homeschool plan', 'unschooling approach',
)

_TEST_PREP_WORDS = frozenset({
    "sat", "act", "gre", "gmat", "lsat", "mcat", "toefl", "ielts", "practice-test", "prep",
})
_TEST_PREP_PHRASES = (
    'sat prep', 'gre practice', 'mcat study', 'test prep plan', 'practice exam',
)

_BARTENDING_WORDS = frozenset({
    "cocktail", "cocktails", "bartender", "bartending", "shaker", "mixer", "garnish", "neat", "on-the-rocks", "mocktail",
})
_BARTENDING_PHRASES = (
    'cocktail recipe', 'how to bartend', 'make a cocktail', 'bar setup',
)

_TEA_WORDS = frozenset({
    "tea", "teapot", "steep", "matcha", "oolong", "pu-erh", "camellia", "infusion", "gaiwan",
})
_TEA_PHRASES = (
    'brew tea', 'tea ceremony', 'matcha how to', 'tea tasting',
)

_BBQ_WORDS = frozenset({
    "bbq", "barbecue", "smoke", "smoker", "brisket", "ribs", "rub", "pitmaster", "charcoal", "offset",
})
_BBQ_PHRASES = (
    'smoke a brisket', 'bbq ribs', 'smoker temperature', 'barbecue rub',
)

_BEEKEEPING_WORDS = frozenset({
    "beekeeping", "beekeeper", "hive", "honeybee", "honey", "smoker-bee", "queen", "swarm", "varroa", "frames",
})
_BEEKEEPING_PHRASES = (
    'start beekeeping', 'hive inspection', 'bee swarm', 'harvest honey',
)

_AQUARIUM_WORDS = frozenset({
    "aquarium", "fish-tank", "reef", "filter", "nitrates", "ammonia", "cycled", "aquascape", "guppy", "betta",
})
_AQUARIUM_PHRASES = (
    'aquarium setup', 'cycle my tank', 'reef tank', 'fish stocking',
)

_BIRDING_WORDS = frozenset({
    "birding", "birdwatching", "birder", "binoculars", "warbler", "migration", "checklist", "ebird", "plumage",
})
_BIRDING_PHRASES = (
    'bird identification', 'bird watching', 'migration season', 'bird checklist',
)

_HORSES_WORDS = frozenset({
    "horse", "horses", "equestrian", "saddle", "bridle", "dressage", "jumping", "stable", "farrier", "gelding", "mare",
})
_HORSES_PHRASES = (
    'horse care', 'riding lesson', 'saddle fit', 'stable management',
)

_SURVIVAL_WORDS = frozenset({
    "survival", "bushcraft", "wilderness", "shelter", "firecraft", "orienteering", "prepper", "emergency-kit",
})
_SURVIVAL_PHRASES = (
    'survival skills', 'build a shelter', 'wilderness first aid', 'bushcraft basics',
)

_SMART_HOME_WORDS = frozenset({
    "smart-home", "home-assistant", "alexa", "google-home", "hue", "zwave", "zigbee", "automation", "scene",
})
_SMART_HOME_PHRASES = (
    'smart home setup', 'home assistant', 'automate lights', 'smart thermostat',
)

_AUDIO_HIFI_WORDS = frozenset({
    "hifi", "hi-fi", "dac", "amp", "amplifier", "speakers", "turntable", "vinyl", "headphones", "audiophile", "impedance",
})
_AUDIO_HIFI_PHRASES = (
    'hi-fi setup', 'speaker placement', 'dac recommendation', 'vinyl setup',
)

_WATCHES_WORDS = frozenset({
    "watch", "watches", "chronograph", "automatic", "seiko", "rolex", "movement", "horology", "strap", "bezel",
})
_WATCHES_PHRASES = (
    'watch recommendation', 'automatic watch', 'watch care', 'horology basics',
)

_JEWELRY_WORDS = frozenset({
    "jewelry", "jewellery", "ring", "necklace", "bracelet", "pendant", "gold", "silver", "gemstone", "soldering",
})
_JEWELRY_PHRASES = (
    'make jewelry', 'ring size', 'jewelry care', 'custom ring',
)

_CERAMICS_WORDS = frozenset({
    "ceramics", "pottery", "clay", "wheel", "glaze", "kiln", "throwing", "bisque", "slab",
})
_CERAMICS_PHRASES = (
    'pottery wheel', 'glaze recipe', 'ceramics class', 'handbuilding',
)

_CALLIGRAPHY_WORDS = frozenset({
    "calligraphy", "lettering", "nib", "ink", "flourish", "copperplate", "brush-lettering", "hand-lettering",
})
_CALLIGRAPHY_PHRASES = (
    'calligraphy practice', 'learn calligraphy', 'brush lettering', 'pointed pen',
)

_LEGO_WORDS = frozenset({
    "lego", "brick", "moc", "minifig", "technic", "set", "instructions", "afol",
})
_LEGO_PHRASES = (
    'lego moc', 'lego build', 'technic set', 'lego sorting',
)

_HAM_RADIO_WORDS = frozenset({
    "ham", "amateur-radio", "hf", "vhf", "uhf", "antenna", "transceiver", "qso", "cw", "ft8", "sota",
})
_HAM_RADIO_PHRASES = (
    'ham radio license', 'antenna setup', 'hf radio', 'amateur radio',
)

_QUANT_WORDS = frozenset({
    "quant", "quantitative", "backtest", "sharpe", "alpha", "factor", "signal", "pnl", "slippage", "execution",
})
_QUANT_PHRASES = (
    'backtest strategy', 'quant research', 'sharpe ratio', 'factor model',
)

_FRANCHISE_WORDS = frozenset({
    "franchise", "franchising", "franchisor", "franchisee", "royalty", "fdd", "territory", "brand-standards",
})
_FRANCHISE_PHRASES = (
    'buy a franchise', 'franchise disclosure', 'franchise fees', 'franchise ops',
)

_RESTAURANT_WORDS = frozenset({
    "restaurant", "menu", "kitchen", "foh", "boh", "covers", "turnover", "chef", "line-cook", "reservations",
})
_RESTAURANT_PHRASES = (
    'restaurant menu', 'open a restaurant', 'kitchen workflow', 'restaurant labor',
)

_PROPERTY_MGMT_WORDS = frozenset({
    "property-management", "landlord", "tenant", "lease", "rent", "maintenance-request", "vacancy", "turnover",
})
_PROPERTY_MGMT_PHRASES = (
    'property management', 'tenant screening', 'lease renewal', 'rental maintenance',
)

_PLUMBING_WORDS = frozenset({
    "plumbing", "plumber", "pipe", "drain", "toilet", "faucet", "water-heater", "p-trap", "pvc", "leak",
})
_PLUMBING_PHRASES = (
    'fix a leak', 'clogged drain', 'replace faucet', 'water heater',
)

_ELECTRICAL_TRADE_WORDS = frozenset({
    "electrical", "electrician", "wiring", "breaker", "outlet", "circuit", "gfci", "voltage", "conduit", "panel",
})
_ELECTRICAL_TRADE_PHRASES = (
    'electrical outlet', 'breaker tripped', 'wire a switch', 'electrical code',
)

_HVAC_WORDS = frozenset({
    "hvac", "furnace", "air-conditioner", "ac", "heat-pump", "thermostat", "ductwork", "filter", "refrigerant",
})
_HVAC_PHRASES = (
    'hvac maintenance', 'heat pump', 'ac not cooling', 'replace filter',
)

_MOVING_WORDS = frozenset({
    "moving", "relocation", "pack", "packing", "movers", "boxes", "inventory", "change-of-address",
})
_MOVING_PHRASES = (
    'moving checklist', 'pack for move', 'hire movers', 'change of address',
)

_DECLUTTER_WORDS = frozenset({
    "declutter", "decluttering", "organize", "organizing", "konmari", "minimalism", "closet", "storage", "tidy",
})
_DECLUTTER_PHRASES = (
    'declutter home', 'organize closet', 'konmari method', 'minimalist home',
)

_DIGITAL_NOMAD_WORDS = frozenset({
    "digital-nomad", "nomad", "coworking", "long-stay", "visa-run", "timezone-hopping", "remote-travel",
})
_DIGITAL_NOMAD_PHRASES = (
    'digital nomad', 'work while traveling', 'nomad setup', 'long stay visa',
)

_EXPAT_WORDS = frozenset({
    "expat", "expatriate", "abroad", "relocate-abroad", "cost-of-living", "local-bank", "residency",
})
_EXPAT_PHRASES = (
    'move abroad', 'expat life', 'cost of living abroad', 'expat banking',
)

_NEURODIVERSITY_WORDS = frozenset({
    "adhd", "autism", "autistic", "neurodivergent", "neurodiversity", "sensory", "executive-function", "masking", "stimming",
})
_NEURODIVERSITY_PHRASES = (
    'adhd tips', 'autism support', 'executive function', 'sensory friendly',
)

_DISABILITY_WORDS = frozenset({
    "disability", "disabled", "wheelchair", "accommodation", "ada", "assistive", "accessibility-need", "mobility",
})
_DISABILITY_PHRASES = (
    'disability accommodation', 'ada request', 'assistive tech', 'mobility aid',
)

_PHYSICAL_THERAPY_WORDS = frozenset({
    "physical-therapy", "physiotherapy", "pt", "rehab", "range-of-motion", "stretch", "strengthening", "injury-recovery",
})
_PHYSICAL_THERAPY_PHRASES = (
    'physical therapy exercises', 'rehab plan', 'after injury', 'pt exercises',
)

_OPTOMETRY_WORDS = frozenset({
    "optometry", "optometrist", "glasses", "contacts", "prescription-lenses", "myopia", "astigmatism", "eye-exam",
})
_OPTOMETRY_PHRASES = (
    'eye exam', 'glasses prescription', 'contact lenses', 'vision care',
)

_GENETICS_WORDS = frozenset({
    "genetics", "genome", "dna-test", "allele", "mutation", "crispr", "genotype", "phenotype", "sequencing",
})
_GENETICS_PHRASES = (
    'genetics basics', 'how inheritance works', 'crispr explained', 'genome sequencing',
)

_BIOTECH_WORDS = frozenset({
    "biotech", "biotechnology", "bioprocess", "fermentation-bio", "cell-culture", "assay", "gmp", "cdmo",
})
_BIOTECH_PHRASES = (
    'biotech career', 'bioprocess basics', 'cell culture', 'biotech startup',
)

_SPACEFLIGHT_WORDS = frozenset({
    "spaceflight", "rocket", "rocketry", "orbit", "orbital", "payload", "launch", "spacecraft", "propellant", "delta-v",
})
_SPACEFLIGHT_PHRASES = (
    'rocket equation', 'orbital mechanics', 'launch vehicle', 'space mission',
)

_URBAN_PLANNING_WORDS = frozenset({
    "urban-planning", "zoning", "transit", "walkability", "housing-supply", "complete-streets", "land-use", "metro",
})
_URBAN_PLANNING_PHRASES = (
    'urban planning', 'zoning code', 'transit oriented', 'city design',
)

_DJ_WORDS = frozenset({
    "dj", "djing", "mix", "mixing", "serato", "rekordbox", "controller", "turntables", "setlist", "drop",
})
_DJ_PHRASES = (
    'dj mix', 'learn to dj', 'dj set', 'beatmatching',
)

_GUITAR_WORDS = frozenset({
    "guitar", "guitarist", "fretboard", "chords", "tabs", "tab", "strum", "picking", "amp-guitar", "pedal",
})
_GUITAR_PHRASES = (
    'guitar lesson', 'learn guitar', 'chord progression guitar', 'guitar tabs',
)

_PIANO_WORDS = frozenset({
    "piano", "pianist", "keyboard", "scales", "arpeggio", "pedaling", "sight-reading", "repertoire",
})
_PIANO_PHRASES = (
    'piano lesson', 'learn piano', 'piano practice', 'sight reading',
)

_SINGING_WORDS = frozenset({
    "singing", "vocal", "vocals", "singer", "breath-support", "range", "warm-up", "choir", "pitch",
})
_SINGING_PHRASES = (
    'singing lesson', 'vocal warm up', 'improve singing', 'find my range',
)

_IMPROV_WORDS = frozenset({
    "improv", "improvisation", "yes-and", "scene-work", "short-form", "long-form", "harold",
})
_IMPROV_PHRASES = (
    'improv class', 'yes and', 'improv scene', 'short form improv',
)

_FACILITATION_WORDS = frozenset({
    "facilitate", "facilitation", "workshop", "icebreaker", "agenda", "breakout", "retro-facilitation",
})
_FACILITATION_PHRASES = (
    'facilitate a workshop', 'meeting facilitation', 'workshop agenda', 'group facilitation',
)

_UNION_WORDS = frozenset({
    "union", "unions", "collective-bargaining", "strike", "grievance", "labor", "shop-steward", "nlrb",
})
_UNION_PHRASES = (
    'form a union', 'collective bargaining', 'union contract', 'labor rights',
)

_CAMPAIGN_WORDS = frozenset({
    "campaign", "canvass", "door-knocking", "get-out-the-vote", "gotv", "field-organizing", "endorsement", "ballot",
})
_CAMPAIGN_PHRASES = (
    'political campaign', 'get out the vote', 'canvass script', 'campaign plan',
)

_COCKTAILS_WORDS = frozenset({
    "mixology", "old-fashioned", "negroni", "martini", "sour", "bitters", "simple-syrup", "stirred", "shaken",
})
_COCKTAILS_PHRASES = (
    'classic cocktail', 'make a negroni', 'old fashioned recipe', 'mixology basics',
)

_FERMENTATION_WORDS = frozenset({
    "ferment", "fermentation", "kombucha", "kefir", "kimchi", "sauerkraut", "scoby", "brine", "lacto",
})
_FERMENTATION_PHRASES = (
    'ferment vegetables', 'make kombucha', 'kimchi recipe', 'sourdough starter',
)

_FORAGING_WORDS = frozenset({
    "foraging", "forage", "wild-edible", "mushroom-id", "morel", "ramps", "ethical-forage",
})
_FORAGING_PHRASES = (
    'foraging guide', 'wild edibles', 'identify mushroom', 'forage safely',
)

_MYCOLOGY_WORDS = frozenset({
    "mycology", "mushroom", "mushrooms", "mycelium", "spore", "fruiting", "substrate", "spawn",
})
_MYCOLOGY_PHRASES = (
    'grow mushrooms', 'mushroom identification', 'mycelium culture', 'fruiting chamber',
)

_PERMACULTURE_WORDS = frozenset({
    "permaculture", "food-forest", "guild", "swale", "companion-planting", "regenerative", "homestead",
})
_PERMACULTURE_PHRASES = (
    'permaculture design', 'food forest', 'permaculture garden', 'homestead plan',
)

_TINY_HOME_WORDS = frozenset({
    "tiny-home", "tiny-house", "vanlife", "rv", "small-space", "minimal-living", "skoolie",
})
_TINY_HOME_PHRASES = (
    'tiny house', 'van life', 'tiny home build', 'small space living',
)

_HOME_THEATER_WORDS = frozenset({
    "home-theater", "projector", "surround", "avr", "dolby", "atmos", "screen", "calibration", "hdmi",
})
_HOME_THEATER_PHRASES = (
    'home theater setup', 'projector screen', 'surround sound', 'avr setup',
)

_STREAMING_WORDS = frozenset({
    "streaming", "stream", "obs", "twitch", "streamlabs", "bitrate", "overlay", "facecam", "vod",
})
_STREAMING_PHRASES = (
    'streaming setup', 'obs scenes', 'twitch stream', 'improve stream',
)

_OPEN_SOURCE_WORDS = frozenset({
    "open-source", "oss", "contributor", "maintainers", "license", "mit", "apache", "code-of-conduct", "upstream",
})
_OPEN_SOURCE_PHRASES = (
    'contribute open source', 'open source license', 'first contribution', 'maintainer guide',
)

_DOCUMENTATION_SITE_WORDS = frozenset({
    "docusaurus", "mkdocs", "sphinx", "docs-site", "versioned-docs", "nav", "sidebar",
})
_DOCUMENTATION_SITE_PHRASES = (
    'docs site', 'documentation portal', 'version docs', 'docs information architecture',
)

_OBSERVABILITY_WORDS = frozenset({
    "observability", "prometheus", "grafana", "opentelemetry", "tracing", "metrics", "logs", "alert", "dashboard",
})
_OBSERVABILITY_PHRASES = (
    'observability stack', 'prometheus metrics', 'distributed tracing', 'grafana dashboard',
)

_PLATFORM_ENG_WORDS = frozenset({
    "platform-engineering", "developer-experience", "dx", "internal-platform", "paved-road", "self-service", "golden-path",
})
_PLATFORM_ENG_PHRASES = (
    'platform engineering', 'developer experience', 'internal developer platform', 'paved road',
)

_PRODUCT_MARKETING_WORDS = frozenset({
    "product-marketing", "pmm", "go-to-market", "gtm", "launch", "battlecard", "competitive", "enablement",
})
_PRODUCT_MARKETING_PHRASES = (
    'product launch', 'go to market', 'battlecard', 'product marketing',
)

_COPYWRITING_WORDS = frozenset({
    "copywriting", "copy", "headline", "cta", "landing-page", "sales-page", "value-prop", "hook",
})
_COPYWRITING_PHRASES = (
    'write copy', 'landing page copy', 'headline options', 'call to action',
)

_AFFILIATE_WORDS = frozenset({
    "affiliate", "affiliate-marketing", "commission", "tracking-link", "disclosure", "cpa", "revshare",
})
_AFFILIATE_PHRASES = (
    'affiliate marketing', 'affiliate program', 'commission structure', 'affiliate disclosure',
)

_AMAZON_FBA_WORDS = frozenset({
    "fba", "amazon-seller", "asin", "seller-central", "ppc-amazon", "inventory", "fulfillment",
})
_AMAZON_FBA_PHRASES = (
    'amazon fba', 'seller central', 'amazon listing', 'fba inventory',
)

_ETSY_WORDS = frozenset({
    "etsy", "handmade", "shop-listing", "craft-fair", "seo-etsy", "digital-download",
})
_ETSY_PHRASES = (
    'etsy shop', 'etsy listing', 'handmade business', 'etsy seo',
)

_GRANT_WRITING_WORDS = frozenset({
    "grant", "grants", "rfp-grant", "foundation", "proposal", "budget-narrative", "outcomes", "funder",
})
_GRANT_WRITING_PHRASES = (
    'grant proposal', 'write a grant', 'foundation grant', 'grant budget',
)

_BOARD_GOVERNANCE_WORDS = frozenset({
    "board", "governance", "fiduciary", "bylaws", "minutes", "committee", "nonprofit-board", "director",
})
_BOARD_GOVERNANCE_PHRASES = (
    'board meeting', 'board governance', 'board agenda', 'fiduciary duty',
)

_HIGHER_ED_WORDS = frozenset({
    "college", "university", "admissions", "campus", "registrar", "syllabus", "undergrad", "graduate-school", "fa fsa", "fafsa",
})
_HIGHER_ED_PHRASES = (
    'college admissions', 'choose a college', 'graduate school', 'campus life',
)

_SPECIAL_ED_WORDS = frozenset({
    "iep", "504", "special-education", "inclusion", "accommodations-school", "learning-difference",
})
_SPECIAL_ED_PHRASES = (
    'iep goals', '504 plan', 'special education', 'inclusive classroom',
)

_ESL_WORDS = frozenset({
    "esl", "efl", "ell", "english-learner", "tefl", "tesol", "grammar-esl", "pronunciation",
})
_ESL_PHRASES = (
    'esl lesson', 'teach english', 'english learner', 'efl classroom',
)

_MEETING_WORDS = frozenset({
    "meeting", "agenda", "minutes", "notes", "action-items", "decision-log", "standing-meeting",
})
_MEETING_PHRASES = (
    'meeting agenda', 'meeting notes', 'run a meeting', 'action items',
)

_OKRS_WORDS = frozenset({
    "okr", "okrs", "objectives", "key-results", "alignment", "cascading",
})
_OKRS_PHRASES = (
    'write okrs', 'okr examples', 'set okrs', 'key results',
)

_CHANGE_MGMT_WORDS = frozenset({
    "change-management", "adoption", "rollout", "stakeholder-map", "resistance", "training-plan",
})
_CHANGE_MGMT_PHRASES = (
    'change management', 'org change', 'rollout plan', 'adoption plan',
)

_VC_WORDS = frozenset({
    "venture", "vc", "angel", "term-sheet", "cap-table", "dilution", "seed", "series-a", "due-diligence",
})
_VC_PHRASES = (
    'term sheet', 'seed round', 'angel invest', 'cap table', 'due diligence',
)

_CROWDFUNDING_WORDS = frozenset({
    "crowdfunding", "kickstarter", "indiegogo", "backers", "campaign-page", "stretch-goal", "pledges",
})
_CROWDFUNDING_PHRASES = (
    'kickstarter campaign', 'crowdfunding page', 'stretch goals', 'launch campaign',
)

_DISASTER_PREP_WORDS = frozenset({
    "disaster", "emergency-prep", "go-bag", "evacuation", "earthquake-prep", "hurricane-prep", "supply-kit",
})
_DISASTER_PREP_PHRASES = (
    'emergency kit', 'disaster plan', 'go bag', 'evacuation plan',
)

_HUMANITARIAN_WORDS = frozenset({
    "humanitarian", "aid", "relief", "refugee", "ngo-field", "disaster-response", "shelter-cluster",
})
_HUMANITARIAN_PHRASES = (
    'humanitarian aid', 'disaster response', 'refugee support', 'relief logistics',
)

_LOCAL_GOV_WORDS = frozenset({
    "city-council", "mayor", "municipal", "town-hall", "ordinance", "public-comment", "zoning-board",
})
_LOCAL_GOV_PHRASES = (
    'city council', 'town hall', 'municipal services', 'public comment',
)

_HOUSING_WORDS = frozenset({
    "housing", "affordable-housing", "rent-burden", "eviction", "shelter", "housing-policy", "section-8",
})
_HOUSING_PHRASES = (
    'affordable housing', 'housing policy', 'rental assistance', 'housing crisis',
)

_FOOD_SECURITY_WORDS = frozenset({
    "food-security", "food-bank", "food-insecurity", "snap", "pantry", "mutual-aid", "meal-program",
})
_FOOD_SECURITY_PHRASES = (
    'food bank', 'food insecurity', 'community pantry', 'meal program',
)

_ZERO_WASTE_WORDS = frozenset({
    "zero-waste", "waste-reduction", "reuse", "recycling", "compost", "plastic-free", "refill",
})
_ZERO_WASTE_PHRASES = (
    'zero waste', 'reduce waste', 'plastic free', 'compost at home',
)

_COMPOSTING_WORDS = frozenset({
    "compost", "composting", "compost-bin", "vermicompost", "bokashi", "browns", "greens", "carbon-nitrogen",
})
_COMPOSTING_PHRASES = (
    'start composting', 'compost bin', 'compost ratio', 'worm bin',
)

_SOLAR_HOME_WORDS = frozenset({
    "solar-panels", "rooftop-solar", "inverter", "net-metering", "battery-storage", "kwh", "photovoltaic",
})
_SOLAR_HOME_PHRASES = (
    'home solar', 'solar panels', 'battery storage', 'net metering',
)

_EV_WORDS = frozenset({
    "ev", "electric-vehicle", "charging", "level-2", "supercharger", "range-anxiety", "kwh-per-mile", "home-charger",
})
_EV_PHRASES = (
    'ev charging', 'electric car', 'home charger', 'ev range',
)

_MOTORSPORTS_WORDS = frozenset({
    "motorsport", "motorsports", "racing", "track-day", "lap-time", "telemetry", "paddock", "formula",
})
_MOTORSPORTS_PHRASES = (
    'track day', 'racing line', 'motorsports', 'lap time',
)

_SKATEBOARDING_WORDS = frozenset({
    "skateboard", "skateboarding", "skate", "ollie", "kickflip", "halfpipe", "street-skating", "trucks",
})
_SKATEBOARDING_PHRASES = (
    'learn to skate', 'ollie tutorial', 'skateboard setup', 'skate tricks',
)

_SURFING_WORDS = frozenset({
    "surf", "surfing", "surfer", "wave", "board", "wetsuit", "paddle", "break", "swell",
})
_SURFING_PHRASES = (
    'learn to surf', 'surf forecast', 'surfboard choice', 'surf etiquette',
)

_KAYAKING_WORDS = frozenset({
    "kayak", "kayaking", "paddle", "pf d", "pfd", "river", "whitewater", "sea-kayak", "roll",
})
_KAYAKING_PHRASES = (
    'kayak trip', 'learn to kayak', 'whitewater kayak', 'sea kayaking',
)

_ROWING_WORDS = frozenset({
    "rowing", "row", "erg", "ergometer", "crew", "sweep", "scull", "coxswain", "split",
})
_ROWING_PHRASES = (
    'rowing technique', 'erg workout', 'crew practice', 'sculling',
)

_TRIATHLON_WORDS = frozenset({
    "triathlon", "triathlete", "ironman", "brick-workout", "transition", "t1", "t2", "half-iron",
})
_TRIATHLON_PHRASES = (
    'triathlon training', 'brick workout', 'ironman plan', 'race transition',
)

_POWERLIFTING_WORDS = frozenset({
    "powerlifting", "powerlifter", "1rm", "meet", "squat", "bench", "deadlift", "peaking", "rpe",
})
_POWERLIFTING_PHRASES = (
    'powerlifting program', '1rm calculator', 'meet prep', 'bench press program',
)

_BODYBUILDING_WORDS = frozenset({
    "bodybuilding", "bodybuilder", "hypertrophy", "posing", "cutting", "bulking", "contest-prep", "aesthetics",
})
_BODYBUILDING_PHRASES = (
    'bodybuilding program', 'contest prep', 'bulking plan', 'posing practice',
)

_CALISTHENICS_WORDS = frozenset({
    "calisthenics", "bodyweight", "pull-up", "push-up", "handstand", "muscle-up", "rings", "progression",
})
_CALISTHENICS_PHRASES = (
    'calisthenics program', 'muscle up', 'handstand progress', 'pull up program',
)

_PARKOUR_WORDS = frozenset({
    "parkour", "freerunning", "vault", "precision-jump", "roll", "traceur",
})
_PARKOUR_PHRASES = (
    'parkour training', 'learn parkour', 'precision jump', 'vault technique',
)

_TENNIS_WORDS = frozenset({
    "tennis", "forehand", "backhand", "serve", "volley", "rally", "racket", "racquet", "baseline", "deuce", "tiebreak",
})
_TENNIS_PHRASES = (
    'tennis lesson', 'improve serve', 'tennis strategy', 'forehand technique',
)

_BASKETBALL_WORDS = frozenset({
    "basketball", "hoops", "dribble", "layup", "three-pointer", "rebound", "defense", "offense", "nba", "pickup",
})
_BASKETBALL_PHRASES = (
    'basketball drills', 'shooting form', 'basketball workout', 'zone defense',
)

_SOCCER_WORDS = frozenset({
    "soccer", "football", "striker", "midfielder", "goalkeeper", "dribbling", "crossing", "set-piece", "formation", "pitch",
})
_SOCCER_PHRASES = (
    'soccer drills', 'football tactics', 'improve dribbling', 'set piece routine',
)

_BASEBALL_WORDS = frozenset({
    "baseball", "pitching", "batting", "hitting", "catcher", "infield", "outfield", "fastball", "curveball", "mlb",
})
_BASEBALL_PHRASES = (
    'batting practice', 'pitching mechanics', 'baseball drills', 'hitting tips',
)

_HOCKEY_WORDS = frozenset({
    "hockey", "puck", "slapshot", "powerplay", "goalie", "rink", "icing", "offside", "nhl", "skate",
})
_HOCKEY_PHRASES = (
    'hockey drills', 'slapshot technique', 'hockey skating', 'power play',
)

_VOLLEYBALL_WORDS = frozenset({
    "volleyball", "serve", "set", "spike", "block", "libero", "rotation", "dig", "ace",
})
_VOLLEYBALL_PHRASES = (
    'volleyball drills', 'serve receive', 'spike technique', 'volleyball rotation',
)

_BOXING_WORDS = frozenset({
    "boxing", "jab", "cross", "hook", "uppercut", "sparring", "heavy-bag", "footwork", "guard",
})
_BOXING_PHRASES = (
    'boxing workout', 'jab cross', 'sparring tips', 'heavy bag routine',
)

_WRESTLING_WORDS = frozenset({
    "wrestling", "takedown", "pin", "escape", "reversal", "folkstyle", "freestyle", "mat",
})
_WRESTLING_PHRASES = (
    'wrestling drills', 'takedown technique', 'wrestling practice', 'escape move',
)

_FENCING_WORDS = frozenset({
    "fencing", "foil", "epee", "sabre", "parry", "riposte", "lunge", "piste", "bout",
})
_FENCING_PHRASES = (
    'fencing lesson', 'parry riposte', 'fencing footwork', 'epee strategy',
)

_ARCHERY_WORDS = frozenset({
    "archery", "bow", "arrow", "recurve", "compound", "draw", "anchor", "fletching", "target",
})
_ARCHERY_PHRASES = (
    'archery form', 'bow setup', 'archery practice', 'improve aim',
)

_SAILING_WORDS = frozenset({
    "sailing", "sailboat", "keel", "jib", "mainsail", "tacking", "jibing", "knot", "marina", "regatta",
})
_SAILING_PHRASES = (
    'learn to sail', 'sailing lesson', 'points of sail', 'docking a sailboat',
)

_HIKING_WORDS = frozenset({
    "hiking", "hike", "trail", "backpack", "elevation", "switchback", "trekking", "day-hike", "summit",
})
_HIKING_PHRASES = (
    'hiking trail', 'day hike plan', 'hiking gear', 'backpack packing',
)

_CAMPING_WORDS = frozenset({
    "camping", "campsite", "tent", "sleeping-bag", "campfire", "campstove", "car-camping", "dispersed",
})
_CAMPING_PHRASES = (
    'camping checklist', 'pitch a tent', 'camping gear', 'car camping',
)

_BACKPACKING_WORDS = frozenset({
    # Never include bare "at" (Appalachian Trail) — it matches the English
    # preposition and flips “look at the results” into Backpacking mode.
    "backpacking", "thru-hike", "ultralight", "base-weight", "trail-legs",
    "resupply", "pct", "appalachian",
})
_BACKPACKING_PHRASES = (
    'backpacking trip', 'ultralight gear', 'thru hike', 'base weight',
    'appalachian trail', 'at thru-hike', 'section hike on the at',
)

_CROSSFIT_WORDS = frozenset({
    # No bare "box" — matches “dialog box”, “text box”, etc.
    "crossfit", "wod", "amrap", "emom", "rx", "scaling", "metcon", "thruster",
})
_CROSSFIT_PHRASES = (
    'crossfit wod', 'scale this wod', 'crossfit program', 'amrap workout',
    'crossfit box', 'garage gym box',
)

_PILATES_WORDS = frozenset({
    # No bare "core" / "hundred" — everyday English (“core of the issue”).
    "pilates", "reformer", "mat-pilates", "plank-series", "contrology",
})
_PILATES_PHRASES = (
    'pilates workout', 'reformer pilates', 'pilates core', 'mat pilates',
    'the hundred pilates', 'pilates hundred',
)

_GYMNASTICS_WORDS = frozenset({
    "gymnastics", "gymnast", "vault", "bars", "beam", "floor", "handstand", "tumbling", "rings",
})
_GYMNASTICS_PHRASES = (
    'gymnastics skills', 'handstand hold', 'tumbling progress', 'bars training',
)

_ICE_SKATING_WORDS = frozenset({
    "ice-skating", "figure-skating", "edges", "spin", "axel", "rink", "blades", "crossovers",
})
_ICE_SKATING_PHRASES = (
    'ice skating lesson', 'figure skating', 'learn edges', 'axel progress',
)

_OLYMPIC_LIFTING_WORDS = frozenset({
    "olympic-lifting", "snatch", "clean-and-jerk", "clean", "jerk", "weightlifting", "barbell-cycling",
})
_OLYMPIC_LIFTING_PHRASES = (
    'snatch technique', 'clean and jerk', 'olympic lifting program', 'weightlifting session',
)

_DRUMS_WORDS = frozenset({
    "drums", "drum", "drummer", "snare", "kick", "hi-hat", "fill", "groove", "metronome", "rudiment",
})
_DRUMS_PHRASES = (
    'drum lesson', 'learn drums', 'drum fill', 'rudiment practice',
)

_BASS_WORDS = frozenset({
    "bass", "bassline", "bassist", "fingerstyle", "slap", "walking-bass", "low-end",
})
_BASS_PHRASES = (
    'bass lesson', 'bassline ideas', 'slap bass', 'walking bass',
)

_VIOLIN_WORDS = frozenset({
    "violin", "viola", "cello", "bow", "fingering", "vibrato", "shifting", "orchestra", "etude",
})
_VIOLIN_PHRASES = (
    'violin lesson', 'learn violin', 'bow technique', 'violin practice',
)

_MUSIC_PRODUCTION_WORDS = frozenset({
    "daw", "ableton", "logic-pro", "fl-studio", "mixing", "mastering", "plugin", "vst", "arrangement", "midi",
})
_MUSIC_PRODUCTION_PHRASES = (
    'mix this track', 'ableton project', 'music production', 'mastering basics',
)

_SOUND_DESIGN_WORDS = frozenset({
    "sound-design", "synthesis", "synth", "foley", "sfx", "wavetable", "modular", "sampler",
})
_SOUND_DESIGN_PHRASES = (
    'sound design', 'foley recording', 'synth patch', 'game audio',
)

_VOICEOVER_WORDS = frozenset({
    "voiceover", "voice-over", "vo", "narration", "booth", "cold-read", "demo-reel",
})
_VOICEOVER_PHRASES = (
    'voiceover script', 'vo demo', 'narration tips', 'home booth',
)

_SCREENWRITING_WORDS = frozenset({
    "screenplay", "screenwriting", "logline", "treatment", "slugline", "dialogue", "act-structure", "beat-sheet",
})
_SCREENWRITING_PHRASES = (
    'write a screenplay', 'logline help', 'beat sheet', 'scene outline',
)

_NOVEL_WORDS = frozenset({
    "novel", "manuscript", "chapter", "protagonist", "plot", "subplot", "nanowrimo", "draft", "revision",
})
_NOVEL_PHRASES = (
    'write a novel', 'novel outline', 'chapter revision', 'plot structure',
)

_BLOGGING_WORDS = frozenset({
    "blog", "blogging", "blog-post", "wordpress", "substack-post", "newsletter-post", "permalink",
})
_BLOGGING_PHRASES = (
    'write a blog post', 'blog outline', 'blogging strategy', 'wordpress post',
)

_JOURNALING_WORDS = frozenset({
    "journal", "journaling", "diary", "morning-pages", "prompt", "reflection", "bullet-journal",
})
_JOURNALING_PHRASES = (
    'journal prompts', 'morning pages', 'bullet journal', 'reflective writing',
)

_TRANSLATION_WORDS = frozenset({
    "translate", "translation", "translator", "localization", "l10n", "i18n", "source-text", "target-language",
})
_TRANSLATION_PHRASES = (
    'translate this', 'localization guide', 'translation notes', 'localize copy',
)

_SIGN_LANGUAGE_WORDS = frozenset({
    "asl", "sign-language", "signed", "fingerspelling", "deaf", "interpreter", "classifier",
})
_SIGN_LANGUAGE_PHRASES = (
    'learn asl', 'sign language basics', 'fingerspelling practice', 'asl phrase',
)

_CROCHET_WORDS = frozenset({
    "crochet", "hook", "stitch", "granny-square", "amigurumi", "yarn", "pattern", "sc", "dc",
})
_CROCHET_PHRASES = (
    'crochet pattern', 'learn crochet', 'amigurumi pattern', 'granny square',
)

_EMBROIDERY_WORDS = frozenset({
    "embroidery", "hoop", "floss", "satin-stitch", "cross-stitch", "needle", "motif",
})
_EMBROIDERY_PHRASES = (
    'embroidery pattern', 'cross stitch', 'hand embroidery', 'satin stitch',
)

_QUILTING_WORDS = frozenset({
    "quilt", "quilting", "batting", "piecing", "binding", "block", "fat-quarter", "longarm",
})
_QUILTING_PHRASES = (
    'quilt pattern', 'piecing tutorial', 'binding a quilt', 'beginner quilt',
)

_COSPLAY_WORDS = frozenset({
    "cosplay", "costume", "prop", "wig", "armor", "con", "commission", "worbla",
})
_COSPLAY_PHRASES = (
    'cosplay tutorial', 'make a prop', 'wig styling', 'con prep',
)

_MAGIC_TRICKS_WORDS = frozenset({
    "magic", "magician", "sleight", "card-trick", "illusion", "misdirection", "deck", "vanish",
})
_MAGIC_TRICKS_PHRASES = (
    'card trick', 'learn magic', 'sleight of hand', 'magic routine',
)

_MODEL_BUILDING_WORDS = frozenset({
    "model-kit", "scale-model", "plastic-model", "airbrush", "weathering", "diorama", "gunpla",
})
_MODEL_BUILDING_PHRASES = (
    'model kit', 'scale model', 'weathering techniques', 'gunpla build',
)

_LANDSCAPING_WORDS = frozenset({
    "landscaping", "landscape", "hardscape", "mulch", "sod", "patio", "retaining-wall", "xeriscape",
})
_LANDSCAPING_PHRASES = (
    'landscape design', 'yard makeover', 'hardscape plan', 'mulch beds',
)

_ROOFING_WORDS = frozenset({
    "roof", "roofing", "shingle", "flashing", "gutter", "leak", "ridge", "underlayment",
})
_ROOFING_PHRASES = (
    'roof leak', 'replace shingles', 'gutter cleaning', 'roof inspection',
)

_PAINTING_TRADE_WORDS = frozenset({
    "paint", "painting", "primer", "roller", "brush", "cut-in", "drywall", "spackle", "finish",
})
_PAINTING_TRADE_PHRASES = (
    'paint a room', 'choose paint', 'cut in edges', 'primer tips',
)

_FLOORING_WORDS = frozenset({
    "flooring", "hardwood", "laminate", "vinyl", "tile", "subfloor", "underlayment", "grout",
})
_FLOORING_PHRASES = (
    'install flooring', 'laminate floor', 'tile floor', 'refinish hardwood',
)

_CARPENTRY_WORDS = frozenset({
    "carpentry", "carpenter", "framing", "stud", "joist", "trim", "baseboard", "casing", "miter",
})
_CARPENTRY_PHRASES = (
    'frame a wall', 'install baseboard', 'carpentry project', 'trim work',
)

_APPLIANCE_REPAIR_WORDS = frozenset({
    "appliance", "dishwasher", "washer", "dryer", "refrigerator", "oven", "repair", "error-code",
})
_APPLIANCE_REPAIR_PHRASES = (
    'dishwasher not draining', 'washer repair', 'dryer not heating', 'fridge not cooling',
)

_PEST_CONTROL_WORDS = frozenset({
    "pest", "pests", "ants", "roaches", "mice", "rats", "termites", "bedbugs", "exterminator",
})
_PEST_CONTROL_PHRASES = (
    'get rid of ants', 'mouse in house', 'pest control', 'termite signs',
)

_AUTO_BODY_WORDS = frozenset({
    "auto-body", "bodywork", "dent", "bumper", "clearcoat", "paint-match", "panel", "bondo",
})
_AUTO_BODY_PHRASES = (
    'fix a dent', 'paint match', 'bumper repair', 'auto body work',
)

_DOG_TRAINING_WORDS = frozenset({
    "dog-training", "puppy", "sit", "stay", "leash", "crate", "recall", "obedience", "clicker",
})
_DOG_TRAINING_PHRASES = (
    'train my dog', 'puppy training', 'leash training', 'dog recall',
)

_CAT_CARE_WORDS = frozenset({
    "cat", "cats", "kitten", "litter", "scratching", "meow", "feline", "enrichment",
})
_CAT_CARE_PHRASES = (
    'cat litter', 'kitten care', 'cat behavior', 'cat enrichment',
)

_CHICKENS_WORDS = frozenset({
    "chicken", "chickens", "coop", "hen", "rooster", "eggs", "brooder", "flock", "nesting",
})
_CHICKENS_PHRASES = (
    'chicken coop', 'raise chickens', 'egg laying', 'brooder setup',
)

_REPTILES_WORDS = frozenset({
    "reptile", "reptiles", "snake", "lizard", "gecko", "terrarium", "uvb", "husbandry", "brumation",
})
_REPTILES_PHRASES = (
    'reptile enclosure', 'leopard gecko care', 'snake husbandry', 'uvb lighting',
)

_CAREGIVING_WORDS = frozenset({
    "caregiver", "caregiving", "respite", "care-plan", "adl", "home-care", "burnout",
})
_CAREGIVING_PHRASES = (
    'caregiver tips', 'care plan', 'respite care', 'caregiving schedule',
)

_CHRONIC_ILLNESS_WORDS = frozenset({
    "chronic", "chronic-illness", "flare", "symptom-tracking", "management", "fatigue", "pain-management",
})
_CHRONIC_ILLNESS_PHRASES = (
    'chronic illness', 'symptom tracker', 'flare day plan', 'living with chronic',
)

_MASSAGE_WORDS = frozenset({
    "massage", "bodywork", "trigger-point", "myofascial", "swedish", "deep-tissue", "table",
})
_MASSAGE_PHRASES = (
    'massage technique', 'self massage', 'deep tissue', 'trigger point',
)

_MENTAL_FITNESS_WORDS = frozenset({
    "stress", "mindfulness", "meditation", "breathing", "resilience", "burnout-prevention", "grounding",
})
_MENTAL_FITNESS_PHRASES = (
    'stress management', 'mindfulness practice', 'breathing exercise', 'prevent burnout',
)

_FERTILITY_WORDS = frozenset({
    "fertility", "ivf", "ttc", "ovulation", "cycle-tracking", "conception", "reproductive",
})
_FERTILITY_PHRASES = (
    'fertility basics', 'cycle tracking', 'ttc tips', 'ivf overview',
)

_LACTATION_WORDS = frozenset({
    "lactation", "breastfeeding", "pumping", "latch", "milk-supply", "formula", "ibclc",
})
_LACTATION_PHRASES = (
    'breastfeeding tips', 'pumping schedule', 'latch help', 'milk supply',
)

_PSYCHOLOGY_WORDS = frozenset({
    "psychology", "cognitive", "behavioral", "bias", "memory", "personality", "experiment", "psych",
})
_PSYCHOLOGY_PHRASES = (
    'psychology concept', 'cognitive bias', 'memory research', 'psych study',
)

_NEUROSCIENCE_WORDS = frozenset({
    "neuroscience", "neuron", "synapse", "cortex", "brain", "neuroplasticity", "fmri", "dopamine",
})
_NEUROSCIENCE_PHRASES = (
    'how the brain', 'neuroscience basics', 'neuroplasticity', 'neuron function',
)

_ECONOMICS_WORDS = frozenset({
    "economics", "inflation", "gdp", "supply", "demand", "elasticity", "macro", "micro", "fiscal", "monetary",
})
_ECONOMICS_PHRASES = (
    'explain inflation', 'supply and demand', 'economics basics', 'fiscal policy',
)

_SOCIOLOGY_WORDS = frozenset({
    "sociology", "social-structure", "inequality", "institutions", "qualitative", "ethnography", "class",
})
_SOCIOLOGY_PHRASES = (
    'sociology theory', 'social inequality', 'ethnography basics', 'social institutions',
)

_ANTHROPOLOGY_WORDS = frozenset({
    "anthropology", "ethnography", "culture", "kinship", "fieldwork", "ritual", "material-culture",
})
_ANTHROPOLOGY_PHRASES = (
    'anthropology basics', 'ethnographic methods', 'cultural comparison', 'fieldwork notes',
)

_MATERIALS_SCIENCE_WORDS = frozenset({
    "materials", "alloy", "polymer", "ceramic", "composite", "metallurgy", "tensile", "hardness", "microstructure",
})
_MATERIALS_SCIENCE_PHRASES = (
    'material selection', 'alloy properties', 'polymer basics', 'materials science',
)

_ECOLOGY_WORDS = frozenset({
    "ecology", "ecosystem", "habitat", "biodiversity", "food-web", "species", "conservation", "biomass",
})
_ECOLOGY_PHRASES = (
    'ecosystem basics', 'food web', 'biodiversity', 'habitat restoration',
)

_PERSONAL_FINANCE_WORDS = frozenset({
    "budget", "budgeting", "emergency-fund", "debt", "paycheck", "savings", "net-worth", "cashflow",
})
_PERSONAL_FINANCE_PHRASES = (
    'make a budget', 'pay off debt', 'emergency fund', 'personal finance plan',
)

_RETIREMENT_WORDS = frozenset({
    "retirement", "401k", "ira", "roth", "pension", "social-security", "drawdown", "nest-egg",
})
_RETIREMENT_PHRASES = (
    'retirement plan', '401k basics', 'roth ira', 'retirement savings',
)

_ESTATE_PLANNING_WORDS = frozenset({
    # No bare "will" — matches English future tense (“I will look…”).
    "trust", "estate", "beneficiary", "probate", "power-of-attorney", "executor", "inheritance",
})
_ESTATE_PLANNING_PHRASES = (
    'write a will', 'estate plan', 'living trust', 'power of attorney',
    'last will', 'will and testament', 'my will',
)

_SIDE_HUSTLE_WORDS = frozenset({
    "side-hustle", "side-gig", "extra-income", "freelance-gig", "moonlight", "passive-ish",
})
_SIDE_HUSTLE_PHRASES = (
    'side hustle ideas', 'start a side hustle', 'side gig plan', 'extra income',
)

_REAL_ESTATE_INVESTING_WORDS = frozenset({
    "rental-property", "cap-rate", "cash-on-cash", "brrrr", "house-hack", "noi", "dscr", "syndication",
})
_REAL_ESTATE_INVESTING_PHRASES = (
    'rental property analysis', 'cap rate', 'brrrr strategy', 'house hacking',
)

_IMPORT_EXPORT_WORDS = frozenset({
    "import", "export", "customs", "tariff", "incoterms", "hs-code", "freight-forwarder", "container",
})
_IMPORT_EXPORT_PHRASES = (
    'import goods', 'export checklist', 'customs docs', 'incoterms explained',
)

_INVENTORY_WORDS = frozenset({
    "inventory", "sku", "stock", "reorder", "safety-stock", "warehouse", "cycle-count", "shrinkage",
})
_INVENTORY_PHRASES = (
    'inventory system', 'reorder point', 'cycle count', 'sku management',
)

_DATA_ENGINEERING_WORDS = frozenset({
    "data-engineering", "etl", "elt", "airflow", "dbt", "warehouse", "spark", "pipeline", "lakehouse",
})
_DATA_ENGINEERING_PHRASES = (
    'data pipeline', 'dbt model', 'etl design', 'data warehouse',
)

_SPREADSHEETS_WORDS = frozenset({
    "excel", "spreadsheet", "google-sheets", "vlookup", "xlookup", "pivot", "formula", "csv", "cells",
})
_SPREADSHEETS_PHRASES = (
    'excel formula', 'pivot table', 'google sheets', 'spreadsheet model',
)

_NOCODE_WORDS = frozenset({
    "nocode", "no-code", "low-code", "zapier", "make.com", "airtable", "bubble", "webflow", "automation",
})
_NOCODE_PHRASES = (
    'no code app', 'zapier automation', 'airtable base', 'bubble app',
)

_WORDPRESS_WORDS = frozenset({
    "wordpress", "wp", "woocommerce", "plugin", "theme", "gutenberg", "elementor", "permalinks",
})
_WORDPRESS_PHRASES = (
    'wordpress site', 'install plugin', 'woocommerce setup', 'wordpress theme',
)

_PRIVACY_WORDS = frozenset({
    "privacy", "tracking", "cookies", "gdpr-privacy", "data-minimization", "vpn", "encryption", "anonymity",
})
_PRIVACY_PHRASES = (
    'privacy tips', 'reduce tracking', 'data minimization', 'privacy settings',
)

_PROMPT_ENG_WORDS = frozenset({
    "prompt", "prompting", "prompt-engineering", "system-prompt", "few-shot", "chain-of-thought", "llm-prompt",
})
_PROMPT_ENG_PHRASES = (
    'prompt engineering', 'write a prompt', 'system prompt', 'few shot examples',
)

_KUBERNETES_WORDS = frozenset({
    "kubernetes", "k8s", "pod", "deployment", "service", "ingress", "helm-chart", "kubectl", "namespace",
})
_KUBERNETES_PHRASES = (
    'kubernetes deployment', 'kubectl apply', 'helm chart', 'k8s networking',
)

_GRAPHICS_PROG_WORDS = frozenset({
    "shader", "opengl", "vulkan", "directx", "gpu", "raster", "mesh", "hlsl", "glsl", "raytracing",
})
_GRAPHICS_PROG_PHRASES = (
    'write a shader', 'opengl basics', 'gpu pipeline', 'vulkan tutorial',
)

_COMPILER_WORDS = frozenset({
    "compiler", "lexer", "parser", "ast", "ir", "codegen", "llvm", "interpreter", "bytecode",
})
_COMPILER_PHRASES = (
    'write a compiler', 'parser design', 'llvm ir', 'interpreter basics',
)

_API_DESIGN_WORDS = frozenset({
    "api-design", "rest", "openapi", "swagger", "graphql", "rpc", "versioning", "idempotent", "pagination",
})
_API_DESIGN_PHRASES = (
    'design an api', 'openapi spec', 'rest best practices', 'graphql schema',
)

_FRONTEND_WORDS = frozenset({
    "frontend", "css", "html", "react", "vue", "svelte", "dom", "responsive", "tailwind", "component",
})
_FRONTEND_PHRASES = (
    'frontend component', 'responsive css', 'react component', 'fix layout',
)

_BACKEND_WORDS = frozenset({
    "backend", "server", "api-server", "microservice", "auth", "session", "queue", "worker", "cache",
})
_BACKEND_PHRASES = (
    'backend service', 'design auth', 'message queue', 'api server',
)

_PARENTING_TEENS_WORDS = frozenset({
    "teenager", "teen", "adolescent", "curfew", "high-school", "puberty", "screen-time", "boundaries",
})
_PARENTING_TEENS_PHRASES = (
    'parenting teens', 'talk to my teen', 'teen boundaries', 'screen time teens',
)

_ADOPTION_WORDS = frozenset({
    "adoption", "adopt", "foster", "home-study", "birth-parent", "open-adoption", "agency",
})
_ADOPTION_PHRASES = (
    'adoption process', 'foster care', 'home study', 'open adoption',
)

_DIVORCE_WORDS = frozenset({
    "divorce", "separation", "custody", "mediation", "settlement", "co-parenting", "alimony",
})
_DIVORCE_PHRASES = (
    'divorce process', 'co parenting plan', 'custody basics', 'separation checklist',
)

_GRIEF_WORDS = frozenset({
    "grief", "bereavement", "mourning", "loss", "funeral", "memorial", "condolence",
})
_GRIEF_PHRASES = (
    'grief support', 'condolence message', 'memorial ideas', 'coping with loss',
)

_MINIMALISM_WORDS = frozenset({
    "minimalism", "minimalist", "less-stuff", "intentional", "capsule", "enough",
})
_MINIMALISM_PHRASES = (
    'minimalist lifestyle', 'own less', 'capsule wardrobe', 'intentional living',
)

_LUXURY_WORDS = frozenset({
    "luxury", "haute", "bespoke", "craftsmanship", "heritage", "couture", "fine",
})
_LUXURY_PHRASES = (
    'luxury brand', 'bespoke suit', 'heritage quality', 'luxury care',
)

_THRIFTING_WORDS = frozenset({
    "thrift", "thrifting", "secondhand", "vintage", "consignment", "upcycle", "estate-sale",
})
_THRIFTING_PHRASES = (
    'thrift haul', 'vintage find', 'upcycle clothes', 'consignment tips',
)

_ROAD_TRIP_WORDS = frozenset({
    "road-trip", "roadtrip", "itinerary", "scenic-route", "pit-stop", "car-camping-trip", "mileage",
})
_ROAD_TRIP_PHRASES = (
    'road trip plan', 'scenic route', 'road trip packing', 'drive itinerary',
)

_CRUISE_WORDS = frozenset({
    "cruise", "cruise-ship", "cabin", "itinerary-cruise", "port-day", "excursion", "formal-night",
})
_CRUISE_PHRASES = (
    'cruise planning', 'choose a cabin', 'port day plan', 'cruise packing',
)

_FOOD_TRAVEL_WORDS = frozenset({
    "food-travel", "foodie-trip", "restaurant-reservation", "street-food", "food-tour", "michelin",
})
_FOOD_TRAVEL_PHRASES = (
    'foodie trip', 'street food guide', 'restaurant reservations', 'food tour',
)

_TUTORING_WORDS = frozenset({
    "tutor", "tutoring", "one-on-one", "scaffold", "remediation", "practice-set", "study-session",
})
_TUTORING_PHRASES = (
    'tutoring plan', 'tutor session', 'help my student', 'scaffold this',
)

_CURRICULUM_WORDS = frozenset({
    "curriculum", "scope-and-sequence", "learning-objectives", "unit-plan", "standards", "pacing",
})
_CURRICULUM_PHRASES = (
    'curriculum map', 'unit plan', 'scope and sequence', 'learning objectives',
)

_EARLY_CHILDHOOD_WORDS = frozenset({
    "preschool", "kindergarten", "toddler", "play-based", "early-childhood", "montessori-early", "circle-time",
})
_EARLY_CHILDHOOD_PHRASES = (
    'preschool activities', 'toddler routine', 'kindergarten readiness', 'play based learning',
)

_MONTESSORI_WORDS = frozenset({
    "montessori", "prepared-environment", "practical-life", "sensorial", "work-cycle", "normalization",
})
_MONTESSORI_PHRASES = (
    'montessori at home', 'practical life', 'montessori materials', 'work cycle',
)

_EDTECH_WORDS = frozenset({
    "edtech", "lms", "learning-management", "classroom-tech", "adaptive-learning", "student-data",
})
_EDTECH_PHRASES = (
    'edtech product', 'lms setup', 'classroom technology', 'adaptive learning',
)

_NEIGHBORHOOD_WORDS = frozenset({
    "neighborhood", "hoa", "block", "community-board", "neighbors", "mutual-aid-local", "block-party",
})
_NEIGHBORHOOD_PHRASES = (
    'neighborhood association', 'hoa rules', 'block party', 'neighbor dispute',
)

_VOLUNTEERING_WORDS = frozenset({
    "volunteer", "volunteering", "nonprofit-volunteer", "service-hours", "community-service", "shift",
})
_VOLUNTEERING_PHRASES = (
    'volunteer opportunities', 'volunteer plan', 'community service', 'start volunteering',
)

_FUNDRAISING_EVENTS_WORDS = frozenset({
    "fundraiser", "gala", "auction", "donor", "pledge", "benefit", "silent-auction",
})
_FUNDRAISING_EVENTS_PHRASES = (
    'fundraising event', 'charity gala', 'silent auction', 'donor ask',
)

_PHOTOGRAPHY_EDITING_WORDS = frozenset({
    "lightroom", "photoshop", "raw", "retouch", "color-grade", "curves", "masking", "preset",
})
_PHOTOGRAPHY_EDITING_PHRASES = (
    'edit photos', 'lightroom preset', 'photo retouch', 'color grade photo',
)

_VIDEO_EDITING_WORDS = frozenset({
    "premiere", "final-cut", "davinci", "timeline", "cut", "b-roll", "export", "proxy", "nle",
})
_VIDEO_EDITING_PHRASES = (
    'edit a video', 'premiere timeline', 'davinci resolve', 'export settings',
)

_PODCAST_EDITING_WORDS = frozenset({
    "podcast-edit", "audacity", "descript", "noise-reduction", "loudness", "lufs", "chapter-markers",
})
_PODCAST_EDITING_PHRASES = (
    'edit podcast', 'remove noise', 'podcast loudness', 'episode assembly',
)

_NEWSLETTER_WORDS = frozenset({
    # No bare "issue" — everyday English (“core of the issue”).
    "newsletter", "email-newsletter", "subject-line", "open-rate", "subscriber",
    "beehiiv", "convertkit",
})
_NEWSLETTER_PHRASES = (
    'newsletter issue', 'subject line ideas', 'grow newsletter', 'email newsletter',
)

_COMMUNITY_MGMT_WORDS = frozenset({
    "community", "moderation", "discord-server", "forum", "mods", "guidelines", "engagement",
})
_COMMUNITY_MGMT_PHRASES = (
    'community guidelines', 'moderate discord', 'grow community', 'forum moderation',
)

_CUSTOMER_SUCCESS_WORDS = frozenset({
    "customer-success", "onboarding", "qbr", "churn", "health-score", "renewal", "expansion", "csm",
})
_CUSTOMER_SUCCESS_PHRASES = (
    'customer onboarding', 'reduce churn', 'qbr agenda', 'health score',
)

_REVENUE_OPS_WORDS = frozenset({
    "revops", "revenue-ops", "crm", "salesforce", "hubspot", "pipeline-hygiene", "attribution", "routing",
})
_REVENUE_OPS_PHRASES = (
    'revops process', 'crm hygiene', 'lead routing', 'attribution model',
)

_PEOPLE_OPS_WORDS = frozenset({
    "people-ops", "people-operations", "onboarding-employee", "headcount", "comp-band", "engagement-survey",
})
_PEOPLE_OPS_PHRASES = (
    'people ops', 'employee onboarding', 'comp bands', 'engagement survey',
)

_OFFICE_ADMIN_WORDS = frozenset({
    "admin", "executive-assistant", "calendar", "travel-booking", "expense", "office-ops", "scheduling",
})
_OFFICE_ADMIN_PHRASES = (
    'manage calendar', 'book travel', 'expense report', 'office administration',
)

_RESEARCH_METHODS_WORDS = frozenset({
    "research-methods", "qualitative", "quantitative", "survey", "interview-protocol", "sampling", "validity",
})
_RESEARCH_METHODS_PHRASES = (
    'research design', 'survey design', 'interview protocol', 'sampling strategy',
)

_STATISTICS_APPLIED_WORDS = frozenset({
    "regression", "anova", "p-value", "confidence-interval", "hypothesis-test", "power-analysis", "bayesian",
})
_STATISTICS_APPLIED_PHRASES = (
    'run a regression', 'interpret p value', 'confidence interval', 'hypothesis test',
)

_CLIMATE_ACTION_WORDS = frozenset({
    "climate-action", "decarbonize", "carbon-footprint", "electrify", "advocacy-climate", "net-zero",
})
_CLIMATE_ACTION_PHRASES = (
    'reduce carbon footprint', 'climate action plan', 'electrify home', 'climate advocacy',
)

_RECYCLING_WORDS = frozenset({
    "recycle", "recycling", "contamination", "single-stream", "e-waste", "compostable", "sorting",
})
_RECYCLING_PHRASES = (
    'what can i recycle', 'recycling rules', 'e waste recycle', 'reduce contamination',
)

_WATER_CONSERVATION_WORDS = frozenset({
    "water-conservation", "drought", "low-flow", "greywater", "rain-barrel", "irrigation-timer",
})
_WATER_CONSERVATION_PHRASES = (
    'save water', 'drought tips', 'low flow fixtures', 'greywater basics',
)

_HOME_SECURITY_WORDS = frozenset({
    "home-security", "alarm", "camera", "deadbolt", "motion-light", "safe", "doorbell-cam",
})
_HOME_SECURITY_PHRASES = (
    'home security system', 'better locks', 'camera placement', 'secure home',
)

_CYBER_HYGIENE_WORDS = frozenset({
    "password-manager", "2fa", "mfa", "phishing", "updates", "backup", "passkey", "breach",
})
_CYBER_HYGIENE_PHRASES = (
    'password manager', 'enable 2fa', 'spot phishing', 'backup plan',
)

_PASSWORD_SECURITY_WORDS = frozenset({
    "password", "passwords", "passkey", "credential", "vault", "rotation", "breach-check",
})
_PASSWORD_SECURITY_PHRASES = (
    'strong password', 'passkey setup', 'password manager setup', 'credential hygiene',
)

_BROWSER_EXT_WORDS = frozenset({
    "extension", "browser-extension", "chrome-extension", "firefox-add-on", "permissions", "manifest",
})
_BROWSER_EXT_PHRASES = (
    'chrome extension', 'build extension', 'extension permissions', 'firefox add-on',
)

_EMAIL_PRODUCTIVITY_WORDS = frozenset({
    "inbox", "email", "zero-inbox", "filters", "labels", "templates", "cc", "unsubscribe",
})
_EMAIL_PRODUCTIVITY_PHRASES = (
    'inbox zero', 'email filters', 'email templates', 'triage email',
)

_NOTE_TAKING_WORDS = frozenset({
    "notes", "note-taking", "cornell", "outline-notes", "lecture-notes", "meeting-notes", "highlight",
})
_NOTE_TAKING_PHRASES = (
    'note taking system', 'lecture notes', 'cornell notes', 'meeting notes method',
)

_SPEED_READING_WORDS = frozenset({
    "speed-reading", "reading-speed", "comprehension", "skimming", "scanning", "wpm",
})
_SPEED_READING_PHRASES = (
    'read faster', 'speed reading tips', 'improve comprehension', 'skimming method',
)

_DEBATE_WORDS = frozenset({
    "debate", "rebuttal", "argument", "contention", "cross-ex", "flowing", "resolution",
})
_DEBATE_PHRASES = (
    'debate case', 'write a rebuttal', 'debate structure', 'cross examination',
)

_PUBLIC_POLICY_ANALYSIS_WORDS = frozenset({
    "policy-analysis", "options-memo", "cost-benefit", "stakeholder-analysis", "impact", "recommendation",
})
_PUBLIC_POLICY_ANALYSIS_PHRASES = (
    'policy options memo', 'cost benefit analysis', 'policy recommendation', 'stakeholder analysis',
)

_MAPS_GIS_WORDS = frozenset({
    "gis", "arcgis", "qgis", "shapefile", "geospatial", "projection", "layer", "geocode",
})
_MAPS_GIS_PHRASES = (
    'gis tutorial', 'qgis basics', 'map layers', 'geocode addresses',
)

_CARTOGRAPHY_WORDS = frozenset({
    "cartography", "map-design", "legend", "symbology", "basemap", "choropleth", "labeling",
})
_CARTOGRAPHY_PHRASES = (
    'map design', 'choropleth map', 'cartography tips', 'map legend',
)

_ASTROPHOTOGRAPHY_WORDS = frozenset({
    "astrophotography", "stacking", "tracking-mount", "light-pollution", "deep-sky", "dso", "narrowband",
})
_ASTROPHOTOGRAPHY_PHRASES = (
    'astrophotography setup', 'stacking images', 'deep sky', 'tracking mount',
)

_METEOROLOGY_HOBBY_WORDS = frozenset({
    "forecasting", "radar", "sounding", "skew-t", "storm-spotting", "surface-map", "model-run",
})
_METEOROLOGY_HOBBY_PHRASES = (
    'read a sounding', 'radar interpretation', 'hobby forecasting', 'storm spotting',
)

_AMATEUR_ASTRONOMY_WORDS = frozenset({
    "telescope", "eyepiece", "observing", "star-party", "collimation", "goto-mount", "messier",
})
_AMATEUR_ASTRONOMY_PHRASES = (
    'telescope buying', 'observing plan', 'collimate telescope', 'star party',
)

_BOARD_GAMES_WORDS = frozenset({
    "board-game", "boardgame", "eurogame", "worker-placement", "deckbuilder", "rules-teach", "tabletop-game",
})
_BOARD_GAMES_PHRASES = (
    'board game rules', 'teach a board game', 'worker placement', 'board game design',
)

_PUZZLES_WORDS = frozenset({
    "crossword", "sudoku", "logic-puzzle", "riddle", "cryptic", "puzzle", "nonogram",
})
_PUZZLES_PHRASES = (
    'crossword tips', 'sudoku strategy', 'logic puzzle', 'cryptic crossword',
)

_RUBIKS_WORDS = frozenset({
    "rubiks", "cube", "speedcubing", "cfop", "algorithm", "oll", "pll", "scramble",
})
_RUBIKS_PHRASES = (
    'solve rubiks cube', 'cfop method', 'speedcubing tips', 'oll pll',
)

_ORIGAMI_WORDS = frozenset({
    "origami", "fold", "paper-folding", "crease", "base", "modular-origami", "diagram",
})
_ORIGAMI_PHRASES = (
    'origami tutorial', 'origami crane', 'modular origami', 'folding diagram',
)

_KNIFE_SKILLS_WORDS = frozenset({
    "knife-skills", "julienne", "brunoise", "chiffonade", "honing", "sharpening", "chef-knife",
})
_KNIFE_SKILLS_PHRASES = (
    'knife skills', 'how to chop', 'sharpen knife', 'julienne cut',
)

_MEAL_PREP_WORDS = frozenset({
    "meal-prep", "batch-cook", "prep", "containers", "weekly-meals", "macros-prep", "freezer-meals",
})
_MEAL_PREP_PHRASES = (
    'meal prep plan', 'batch cooking', 'weekly meal prep', 'freezer meals',
)

_KETO_WORDS = frozenset({
    "keto", "ketogenic", "ketosis", "low-carb", "macros", "net-carbs", "electrolytes",
})
_KETO_PHRASES = (
    'keto meal plan', 'enter ketosis', 'keto macros', 'low carb meals',
)

_VEGAN_COOKING_WORDS = frozenset({
    "vegan", "plant-based", "tofu", "tempeh", "aquafaba", "cashew-cream", "veganize",
})
_VEGAN_COOKING_PHRASES = (
    'vegan recipe', 'plant based meal', 'veganize this', 'tofu marinade',
)

_GLUTEN_FREE_WORDS = frozenset({
    "gluten-free", "celiac", "gf", "cross-contact", "rice-flour", "xanthan",
})
_GLUTEN_FREE_PHRASES = (
    'gluten free recipe', 'gf baking', 'celiac cooking', 'avoid cross contact',
)

_SOUS_VIDE_WORDS = frozenset({
    "sous-vide", "immersion-circulator", "water-bath", "vacuum", "sear", "pasteurize-time",
})
_SOUS_VIDE_PHRASES = (
    'sous vide steak', 'sous vide time', 'finish sous vide', 'immersion circulator',
)

_SMOKING_MEAT_WORDS = frozenset({
    "smoke", "smoking", "brisket", "pork-butt", "bark", "stall", "wood-chunks", "probe",
})
_SMOKING_MEAT_PHRASES = (
    'smoke pork butt', 'brisket stall', 'smoking wood', 'probe temperature',
)

_PICKLEBALL_WORDS = frozenset({
    "pickleball", "dink", "kitchen", "paddle", "third-shot", "nvz", "volley-pickle",
})
_PICKLEBALL_PHRASES = (
    'pickleball tips', 'dink practice', 'third shot drop', 'kitchen rules',
)

_BADMINTON_WORDS = frozenset({
    "badminton", "shuttlecock", "smash", "clear", "drop-shot", "racquet-badminton", "doubles",
})
_BADMINTON_PHRASES = (
    'badminton lesson', 'smash technique', 'badminton footwork', 'doubles strategy',
)

_TABLE_TENNIS_WORDS = frozenset({
    "table-tennis", "ping-pong", "pingpong", "paddle-tt", "spin", "loop", "chop", "serve-tt",
})
_TABLE_TENNIS_PHRASES = (
    'table tennis', 'ping pong tips', 'loop drive', 'table tennis serve',
)

_RUGBY_WORDS = frozenset({
    "rugby", "scrum", "lineout", "ruck", "try", "conversion", "tackle-rugby", "union", "league",
})
_RUGBY_PHRASES = (
    'rugby drills', 'scrum technique', 'lineout throws', 'tackle technique rugby',
)

_CRICKET_WORDS = frozenset({
    # No bare "over" — English preposition / “over there”.
    "cricket", "batting", "bowling", "wicket", "spinner", "pace", "fielding", "ashes",
})
_CRICKET_PHRASES = (
    'cricket batting', 'bowling action', 'fielding drills', 'cricket strategy',
    'cricket over', 'maiden over', 'powerplay over',
)

_SOFTBALL_WORDS = frozenset({
    "softball", "fastpitch", "slowpitch", "pitcher", "catcher-softball", "infield-softball",
})
_SOFTBALL_PHRASES = (
    'softball pitching', 'softball hitting', 'fastpitch drills', 'softball practice',
)

_LACROSSE_WORDS = frozenset({
    "lacrosse", "crosse", "cradle", "ground-ball", "faceoff", "clearing", "ride",
})
_LACROSSE_PHRASES = (
    'lacrosse drills', 'cradle technique', 'ground balls', 'faceoff tips',
)

_WATER_POLO_WORDS = frozenset({
    "water-polo", "waterpolo", "eggbeater", "man-up", "hole-set", "counterattack",
})
_WATER_POLO_PHRASES = (
    'water polo drills', 'eggbeater kick', 'water polo offense', 'man up defense',
)

_DIVING_SPORT_WORDS = frozenset({
    "springboard", "platform-diving", "dive-list", "entry", "hurdle", "somersault", "twisting",
})
_DIVING_SPORT_PHRASES = (
    'springboard diving', 'dive list', 'entry technique', 'platform diving',
)

_SYNCHRONIZED_SWIM_WORDS = frozenset({
    "synchronized", "artistic-swimming", "figures", "routine", "scull", "highlight",
})
_SYNCHRONIZED_SWIM_PHRASES = (
    'artistic swimming', 'synchro routine', 'sculling technique', 'figures practice',
)

_EQUESTRIAN_SPORT_WORDS = frozenset({
    "dressage", "show-jumping", "eventing", "equitation", "course-walk", "collection",
})
_EQUESTRIAN_SPORT_PHRASES = (
    'dressage test', 'show jumping course', 'eventing prep', 'equitation tips',
)

_ESPORTS_WORDS = frozenset({
    "esports", "pro-play", "scrim", "vod-review", "aim-train", "tournament", "bracket", "meta",
})
_ESPORTS_PHRASES = (
    'esports practice', 'vod review', 'scrim schedule', 'aim training',
)

_SPEEDRUNNING_WORDS = frozenset({
    "speedrun", "speedrunning", "any%", "glitchless", "splits", "pb", "wr", "route", "rng",
})
_SPEEDRUNNING_PHRASES = (
    'speedrun route', 'split analysis', 'any percent', 'pb attempt',
)

_YOGA_THERAPY_WORDS = frozenset({
    "yoga-therapy", "therapeutic-yoga", "restorative", "yin-therapy", "modifications",
})
_YOGA_THERAPY_PHRASES = (
    'restorative yoga', 'yoga for back', 'therapeutic yoga', 'gentle modifications',
)

_MOBILITY_WORDS = frozenset({
    "mobility", "rom", "range-of-motion", "hip-opener", "shoulder-mobility", "flow-mobility", "cars",
})
_MOBILITY_PHRASES = (
    'mobility routine', 'hip mobility', 'shoulder mobility', 'daily mobility',
)

_BREATHWORK_WORDS = frozenset({
    "breathwork", "box-breathing", "wim-hof", "pranayama-breath", "coherent-breathing", "exhale",
})
_BREATHWORK_PHRASES = (
    'box breathing', 'breathwork practice', 'coherent breathing', 'calm breathing',
)

_SPANISH_WORDS = frozenset({
    "spanish", "español", "español", "castellano", "conjugations", "subjunctive", "por-para",
})
_SPANISH_PHRASES = (
    'learn spanish', 'spanish conjugation', 'how do you say in spanish', 'spanish practice',
)

_FRENCH_WORDS = frozenset({
    "french", "français", "francais", "conjugaison", "subjonctif", "liaison", "tu-vous",
})
_FRENCH_PHRASES = (
    'learn french', 'french conjugation', 'how do you say in french', 'french practice',
)

_GERMAN_WORDS = frozenset({
    "german", "deutsch", "artikel", "akkusativ", "dativ", "trennbar", "umlaute",
})
_GERMAN_PHRASES = (
    'learn german', 'german cases', 'how do you say in german', 'german practice',
)

_JAPANESE_WORDS = frozenset({
    "japanese", "nihongo", "hiragana", "katakana", "kanji", "particles", "keigo", "genki",
})
_JAPANESE_PHRASES = (
    'learn japanese', 'hiragana practice', 'kanji study', 'japanese particles',
)

_MANDARIN_WORDS = frozenset({
    "mandarin", "chinese", "pinyin", "tones", "hanzi", "hsk", "simplified", "traditional-chinese",
})
_MANDARIN_PHRASES = (
    'learn mandarin', 'pinyin tones', 'hsk study', 'chinese characters',
)

_KOREAN_WORDS = frozenset({
    "korean", "hangul", "hangeul", "batchim", "honorifics", "topik", "k-drama-korean",
})
_KOREAN_PHRASES = (
    'learn korean', 'hangul practice', 'korean honorifics', 'topik study',
)

_ITALIAN_WORDS = frozenset({
    "italian", "italiano", "conjugazioni", "passato", "subjunctive-it", "articles-it",
})
_ITALIAN_PHRASES = (
    'learn italian', 'italian conjugation', 'how do you say in italian', 'italian practice',
)

_PORTUGUESE_WORDS = frozenset({
    "portuguese", "português", "brasileiro", "european-portuguese", "conjugação", "tu-você",
})
_PORTUGUESE_PHRASES = (
    'learn portuguese', 'brazilian portuguese', 'portuguese conjugation', 'portuguese practice',
)

_ARABIC_WORDS = frozenset({
    "arabic", "fusha", "msa", "dialect", "harakat", "alif", "calligraphy-arabic",
})
_ARABIC_PHRASES = (
    'learn arabic', 'arabic script', 'msa basics', 'arabic dialect',
)

_HINDI_WORDS = frozenset({
    "hindi", "devanagari", "hinglish", "postpositions", "matras",
})
_HINDI_PHRASES = (
    'learn hindi', 'devanagari practice', 'hindi phrases', 'hindi grammar',
)

_GREEK_LANG_WORDS = frozenset({
    "greek-language", "ellinika", "modern-greek", "alphabet-greek", "cases-greek",
})
_GREEK_LANG_PHRASES = (
    'learn greek', 'modern greek', 'greek alphabet', 'greek phrases',
)

_LATIN_WORDS = frozenset({
    "latin", "declension", "conjugation-latin", "ablative", "cicero", "wheelock", "vulgate",
})
_LATIN_PHRASES = (
    'latin grammar', 'latin translation', 'declension practice', 'wheelock latin',
)

_UKULELE_WORDS = frozenset({
    "ukulele", "uke", "soprano-uke", "concert-uke", "strumming", "c-f-g-am",
})
_UKULELE_PHRASES = (
    'ukulele chords', 'learn ukulele', 'uke songs', 'strumming patterns',
)

_SAXOPHONE_WORDS = frozenset({
    "saxophone", "sax", "alto-sax", "tenor-sax", "reed", "embouchure", "transposition",
})
_SAXOPHONE_PHRASES = (
    'saxophone lesson', 'reed strength', 'sax tone', 'alto sax practice',
)

_TRUMPET_WORDS = frozenset({
    "trumpet", "cornet", "brass", "embouchure-brass", "mouthpiece", "lip-slur", "lead-pipe",
})
_TRUMPET_PHRASES = (
    'trumpet lesson', 'lip slurs', 'trumpet range', 'brass practice',
)

_FLUTE_WORDS = frozenset({
    "flute", "piccolo", "embouchure-flute", "tone-holes", "articulation", "flutist",
})
_FLUTE_PHRASES = (
    'flute lesson', 'flute tone', 'piccolo practice', 'flute articulation',
)

_HARMONICA_WORDS = frozenset({
    "harmonica", "harp", "bending", "positions", "blues-harp", "diatonic", "chromatic-harp",
})
_HARMONICA_PHRASES = (
    'harmonica lesson', 'bending notes', 'blues harmonica', 'harp positions',
)

_BANJO_WORDS = frozenset({
    "banjo", "clawhammer", "scruggs", "rolls", "bluegrass", "frailing", "5-string",
})
_BANJO_PHRASES = (
    'banjo rolls', 'clawhammer lesson', 'bluegrass banjo', 'learn banjo',
)

_MUSIC_THEORY_WORDS = frozenset({
    "music-theory", "harmony", "counterpoint", "scales-theory", "modes", "roman-numerals", "ear-training", "interval",
})
_MUSIC_THEORY_PHRASES = (
    'music theory', 'harmonic analysis', 'ear training', 'scale modes',
)

_GRAPHIC_DESIGN_WORDS = frozenset({
    "graphic-design", "typography", "layout", "grid", "branding-visual", "poster", "adobe", "indesign", "illustrator",
})
_GRAPHIC_DESIGN_PHRASES = (
    'graphic design', 'poster layout', 'typography tips', 'brand identity system',
)

_ILLUSTRATION_WORDS = frozenset({
    "illustration", "illustrator-art", "character-design", "line-art", "inking", "concept-art", "digital-painting",
})
_ILLUSTRATION_PHRASES = (
    'illustration tips', 'character design', 'digital painting', 'concept art',
)

_UX_WRITING_WORDS = frozenset({
    "ux-writing", "microcopy", "empty-state", "error-message", "button-label", "onboarding-copy", "content-design",
})
_UX_WRITING_PHRASES = (
    'ux writing', 'microcopy examples', 'error message copy', 'button labels',
)

_STORYBOARD_WORDS = frozenset({
    "storyboard", "storyboarding", "panel", "animatic", "shot-plan", "thumbnail-story",
})
_STORYBOARD_PHRASES = (
    'storyboard this', 'shot list storyboard', 'animatic plan', 'storyboard panels',
)

_COLOR_GRADING_WORDS = frozenset({
    "color-grading", "lut", "scopes", "rec709", "log", "davinci-color", "primary-grade",
})
_COLOR_GRADING_PHRASES = (
    'color grade', 'create a lut', 'scopes tutorial', 'log to rec709',
)

_LIGHTING_DESIGN_WORDS = frozenset({
    "lighting", "gaffer", "key-light", "fill", "practicals", "gel", "dmx", "cues",
})
_LIGHTING_DESIGN_PHRASES = (
    'lighting setup', 'three point lighting', 'stage lighting', 'lighting cues',
)

_COSTUME_DESIGN_WORDS = frozenset({
    "costume", "wardrobe", "period-costume", "fittings", "fabric-choice", "character-look",
})
_COSTUME_DESIGN_PHRASES = (
    'costume design', 'period costume', 'wardrobe plan', 'character costume',
)

_SET_DESIGN_WORDS = frozenset({
    "set-design", "production-design", "scenic", "props-master", "backdrop", "set-dressing",
})
_SET_DESIGN_PHRASES = (
    'set design', 'production design', 'set dressing', 'scenic design',
)

_PASTRY_WORDS = frozenset({
    "pastry", "croissant", "laminated", "choux", "pate-a-choux", "tart", "ganache", "patisserie",
})
_PASTRY_PHRASES = (
    'croissant recipe', 'choux pastry', 'laminated dough', 'pastry cream',
)

_BREAD_WORDS = frozenset({
    "bread", "loaf", "crumb", "bulk-ferment", "proof", "autolyse", "baker-percent", "scoring",
})
_BREAD_PHRASES = (
    'bake bread', 'sourdough loaf', 'bread formula', 'shaping dough',
)

_CHOCOLATE_WORDS = frozenset({
    "chocolate", "tempering", "cacao", "ganache-choc", "truffle", "couverture", "cocoa-butter",
})
_CHOCOLATE_PHRASES = (
    'temper chocolate', 'chocolate truffles', 'cacao percentage', 'chocolate work',
)

_CHEESE_WORDS = frozenset({
    "cheese", "cheesemaking", "rennet", "culture", "aging", "affineur", "fromage", "curds",
})
_CHEESE_PHRASES = (
    'make cheese', 'cheese pairing', 'aging cheese', 'cheesemaking basics',
)

_CHARCUTERIE_WORDS = frozenset({
    "charcuterie", "salumi", "cure", "nitrite", "prosciutto", "salami", "terrine", "pâté",
})
_CHARCUTERIE_PHRASES = (
    'charcuterie board', 'cure meat', 'salami basics', 'terrine recipe',
)

_PRESERVING_WORDS = frozenset({
    "canning", "pickling", "jam", "jelly", "water-bath", "pressure-canner", "preserves", "brine-pickles",
})
_PRESERVING_PHRASES = (
    'can tomatoes', 'pickle recipe', 'make jam', 'water bath canning',
)

_INDIAN_COOKING_WORDS = frozenset({
    "indian", "curry", "masala", "dal", "dosa", "tandoor", "ghee", "tempering-spices", "biryani",
})
_INDIAN_COOKING_PHRASES = (
    'indian recipe', 'make dal', 'biryani recipe', 'masala blend',
)

_CHINESE_COOKING_WORDS = frozenset({
    "chinese-cooking", "wok", "stir-fry", "soy", "sichuan", "dim-sum", "dumpling", "velveting",
})
_CHINESE_COOKING_PHRASES = (
    'stir fry', 'wok hei', 'dumpling folding', 'sichuan recipe',
)

_MEXICAN_COOKING_WORDS = frozenset({
    "mexican", "salsa", "tortilla", "masa", "mole", "taco", "tamale", "nixtamal", "chile",
})
_MEXICAN_COOKING_PHRASES = (
    'salsa recipe', 'homemade tortillas', 'mole recipe', 'taco night',
)

_ITALIAN_COOKING_WORDS = frozenset({
    "italian-cooking", "pasta", "risotto", "ragu", "pesto", "gnocchi", "osso-buco", "antipasti",
})
_ITALIAN_COOKING_PHRASES = (
    'fresh pasta', 'risotto technique', 'ragu recipe', 'pesto from scratch',
)

_JAPANESE_COOKING_WORDS = frozenset({
    "japanese-cooking", "dashi", "miso", "sushi", "onigiri", "tempura", "donburi", "washoku",
})
_JAPANESE_COOKING_PHRASES = (
    'make dashi', 'sushi rice', 'miso soup', 'onigiri recipe',
)

_BBQ_SAUCES_WORDS = frozenset({
    "bbq-sauce", "rub", "carolina", "kansas-city", "alabama-white", "mop-sauce",
})
_BBQ_SAUCES_PHRASES = (
    'bbq sauce recipe', 'dry rub', 'carolina sauce', 'mop sauce',
)

_COFFEE_ROASTING_WORDS = frozenset({
    "roasting", "roast-profile", "first-crack", "second-crack", "agtron", "green-coffee", "drum-roast",
})
_COFFEE_ROASTING_PHRASES = (
    'roast coffee', 'first crack', 'roast profile', 'green beans',
)

_LATTE_ART_WORDS = frozenset({
    "latte-art", "microfoam", "steaming", "rosetta", "heart-pour", "pitcher", "espresso-art",
})
_LATTE_ART_PHRASES = (
    'latte art', 'microfoam technique', 'pour a rosetta', 'steam milk',
)

_HOUSEPLANTS_WORDS = frozenset({
    "houseplant", "houseplants", "pothos", "monstera", "succulent", "repot", "humidity", "grow-light",
})
_HOUSEPLANTS_PHRASES = (
    'houseplant care', 'repot plant', 'yellow leaves', 'succulent care',
)

_HYDROPONICS_WORDS = frozenset({
    "hydroponics", "hydroponic", "nutrient-solution", "dwc", "nft", "grow-tent", "ppm", "ph-water",
})
_HYDROPONICS_PHRASES = (
    'hydroponic setup', 'nutrient solution', 'dwc system', 'grow tent',
)

_BONSAI_WORDS = frozenset({
    "bonsai", "wiring", "pruning-bonsai", "nebari", "jin", "potting-bonsai", "species-bonsai",
})
_BONSAI_PHRASES = (
    'bonsai care', 'wire a bonsai', 'bonsai pruning', 'repot bonsai',
)

_AQUAPONICS_WORDS = frozenset({
    "aquaponics", "aquaponic", "fish-tank-plants", "media-bed", "raft", "nitrifying",
})
_AQUAPONICS_PHRASES = (
    'aquaponics setup', 'cycle aquaponics', 'media bed', 'fish for aquaponics',
)

_LAWN_CARE_WORDS = frozenset({
    "lawn", "turf", "mowing", "aeration", "overseed", "fertilizer", "thatch", "crabgrass",
})
_LAWN_CARE_PHRASES = (
    'lawn care', 'overseed lawn', 'aerate lawn', 'kill crabgrass',
)

_IRRIGATION_WORDS = frozenset({
    "irrigation", "sprinkler", "drip", "valve", "controller", "zone", "backflow", "emitter",
})
_IRRIGATION_PHRASES = (
    'drip irrigation', 'sprinkler system', 'irrigation timer', 'fix zone',
)

_POOL_CARE_WORDS = frozenset({
    "pool", "chlorine", "ph-pool", "alkalinity", "shock", "filter-pool", "algaecide", "spa",
})
_POOL_CARE_PHRASES = (
    'pool chemistry', 'balance pool', 'shock pool', 'green pool',
)

_FIREPLACE_WORDS = frozenset({
    "fireplace", "chimney", "wood-stove", "flue", "creosote", "damper", "hearth",
})
_FIREPLACE_PHRASES = (
    'fireplace safety', 'clean chimney', 'wood stove', 'start a fire',
)

_DENTAL_HYGIENE_WORDS = frozenset({
    "dental-hygiene", "floss", "plaque", "gingivitis", "toothbrush", "mouthwash", "tartar",
})
_DENTAL_HYGIENE_PHRASES = (
    'flossing technique', 'gum health', 'toothbrush advice', 'reduce plaque',
)

_PHARMACOLOGY_WORDS = frozenset({
    "pharmacology", "mechanism", "receptor", "half-life", "agonist", "antagonist", "cyp", "pharmacokinetics",
})
_PHARMACOLOGY_PHRASES = (
    'drug mechanism', 'pharmacokinetics basics', 'receptor agonist', 'cyp interaction education',
)

_RADIOLOGY_LITERACY_WORDS = frozenset({
    "radiology", "x-ray", "mri", "ct-scan", "ultrasound-imaging", "contrast", "dicom",
})
_RADIOLOGY_LITERACY_PHRASES = (
    'mri basics', 'what is a ct scan', 'x-ray explained', 'imaging overview',
)

_NUTRITION_SCIENCE_WORDS = frozenset({
    "macronutrients", "micronutrients", "bioavailability", "rda", "metabolism", "glycemic", "nutrient",
})
_NUTRITION_SCIENCE_PHRASES = (
    'macronutrient basics', 'glycemic index', 'nutrient density', 'nutrition science',
)

_EPIDEMIOLOGY_WORDS = frozenset({
    "epidemiology", "incidence", "prevalence", "cohort", "case-control", "rr", "odds-ratio", "confounding",
})
_EPIDEMIOLOGY_PHRASES = (
    'incidence vs prevalence', 'cohort study', 'odds ratio', 'epidemiology basics',
)

_BIOSTATISTICS_WORDS = frozenset({
    "biostatistics", "survival-analysis", "kaplan-meier", "hazard-ratio", "clinical-trial-stats", "sample-size",
})
_BIOSTATISTICS_PHRASES = (
    'kaplan meier', 'hazard ratio', 'sample size calculation', 'survival analysis',
)

_BOOKKEEPING_WORDS = frozenset({
    "bookkeeping", "bookkeeper", "reconciliation", "ledger", "chart-of-accounts", "accounts-payable", "receivable",
})
_BOOKKEEPING_PHRASES = (
    'reconcile accounts', 'chart of accounts', 'bookkeeping basics', 'accounts payable',
)

_PAYROLL_WORDS = frozenset({
    "payroll", "paycheck", "withholding", "direct-deposit", "timesheet", "garnishment", "w2-payroll",
})
_PAYROLL_PHRASES = (
    'run payroll', 'payroll taxes overview', 'timesheet system', 'direct deposit setup',
)

_BILLING_WORDS = frozenset({
    "billing", "invoice", "accounts-receivable", "collections", "payment-terms", "dunning", "net-30",
})
_BILLING_PHRASES = (
    'create invoice', 'payment terms', 'collections email', 'accounts receivable',
)

_PRICING_WORDS = frozenset({
    "pricing", "price", "packaging", "tier", "willingness-to-pay", "discounting", "value-metric", "freemium",
})
_PRICING_PHRASES = (
    'pricing strategy', 'pricing tiers', 'value metric', 'discount policy',
)

_SALES_ENABLEMENT_WORDS = frozenset({
    "sales-enablement", "battlecard-sales", "playbook", "demo-script", "collateral", "enablement",
})
_SALES_ENABLEMENT_PHRASES = (
    'sales playbook', 'demo script', 'sales battlecard', 'enablement plan',
)

_PARTNERSHIPS_WORDS = frozenset({
    "partnership", "partnerships", "channel", "reseller", "co-sell", "integration-partner", "mou",
})
_PARTNERSHIPS_PHRASES = (
    'partnership proposal', 'channel strategy', 'co sell motion', 'reseller agreement basics',
)

_CUSTOMER_RESEARCH_WORDS = frozenset({
    "customer-research", "jtbd", "jobs-to-be-done", "discovery-interview", "persona-research", "problem-interview",
})
_CUSTOMER_RESEARCH_PHRASES = (
    'customer interviews', 'jobs to be done', 'discovery interview', 'problem interview',
)

_ANALYTICS_WORDS = frozenset({
    "analytics", "funnel", "conversion", "event-tracking", "ga4", "mixpanel", "amplitude", "kpi-dashboard",
})
_ANALYTICS_PHRASES = (
    'analytics plan', 'event tracking', 'funnel analysis', 'kpi dashboard',
)

_AB_TESTING_WORDS = frozenset({
    "ab-test", "a/b", "experiment", "variant", "control", "significance", "sample-ratio", "mde",
})
_AB_TESTING_PHRASES = (
    'ab test design', 'sample size ab test', 'interpret experiment', 'variant analysis',
)

_RUST_LANG_WORDS = frozenset({
    "rust", "borrow-checker", "lifetime", "cargo", "ownership", "mut", "arc", "mutex-rust", "clippy-rust",
})
_RUST_LANG_PHRASES = (
    'rust ownership', 'borrow checker', 'cargo project', 'rust lifetimes',
)

_GO_LANG_WORDS = frozenset({
    "golang", "goroutine", "channel", "go-mod", "interface-go", "context-go", "go-routine",
})
_GO_LANG_PHRASES = (
    'golang service', 'goroutines channels', 'go modules', 'context cancellation',
)

_PYTHON_DATA_WORDS = frozenset({
    "pandas", "numpy", "dataframe", "jupyter", "matplotlib", "seaborn", "groupby", "csv-python",
})
_PYTHON_DATA_PHRASES = (
    'pandas dataframe', 'groupby pandas', 'jupyter notebook', 'numpy array',
)

_SQL_ANALYTICS_WORDS = frozenset({
    "sql-analytics", "window-function", "cte", "partition-by", "rank", "cohort-sql", "dbt-sql",
})
_SQL_ANALYTICS_PHRASES = (
    'window function', 'sql cte', 'cohort query', 'analytical sql',
)

_TERRAFORM_WORDS = frozenset({
    "terraform", "tf", "hcl", "provider", "state-file", "module-tf", "plan-apply", "workspace-tf",
})
_TERRAFORM_PHRASES = (
    'terraform module', 'terraform state', 'terraform plan', 'hcl configuration',
)

_ANSIBLE_WORDS = frozenset({
    "ansible", "playbook", "role-ansible", "inventory", "handler", "idempotent", "yaml-ansible",
})
_ANSIBLE_PHRASES = (
    'ansible playbook', 'ansible role', 'inventory file', 'idempotent tasks',
)

_CICD_WORDS = frozenset({
    "cicd", "ci-cd", "pipeline", "github-actions", "gitlab-ci", "jenkins-job", "artifact", "deploy-gate",
})
_CICD_PHRASES = (
    'ci cd pipeline', 'github actions workflow', 'deploy pipeline', 'pipeline stages',
)

_DOCKER_WORDS = frozenset({
    "docker", "dockerfile", "container", "compose", "image", "volume", "registry", "entrypoint",
})
_DOCKER_PHRASES = (
    'dockerfile', 'docker compose', 'build image', 'container networking',
)

_LINUX_ADMIN_WORDS = frozenset({
    "linux-admin", "sysadmin", "systemd", "journalctl", "chmod", "chown", "ssh", "cron", "apt", "yum",
})
_LINUX_ADMIN_PHRASES = (
    'systemd service', 'linux permissions', 'ssh config', 'cron job',
)

_NETWORK_SECURITY_WORDS = frozenset({
    "firewall", "ids", "ips", "vpn-security", "segmentation", "zero-trust", "packet-filter", "nacl",
})
_NETWORK_SECURITY_PHRASES = (
    'firewall rules', 'network segmentation', 'zero trust', 'ids monitoring',
)

_PENTEST_DEFENSE_WORDS = frozenset({
    "pentest", "penetration-test", "red-team", "blue-team", "purple-team", "scope", "findings", "remediation",
})
_PENTEST_DEFENSE_PHRASES = (
    'pentest report', 'remediation plan', 'authorized testing', 'security findings',
)

_THREAT_MODEL_WORDS = frozenset({
    "threat-model", "stride", "dfd", "attack-surface", "mitigation", "asset", "trust-boundary",
})
_THREAT_MODEL_PHRASES = (
    'threat model', 'stride analysis', 'attack surface', 'trust boundary',
)

_INCIDENT_RESPONSE_WORDS = frozenset({
    "incident-response", "ir", "containment", "forensics", "ioc", "playbook-ir", "severity", "severity-response",
})
_INCIDENT_RESPONSE_PHRASES = (
    'incident response plan', 'containment steps', 'ir playbook', 'post incident',
)

_QA_TESTING_WORDS = frozenset({
    "qa", "test-case", "regression", "smoke-test", "exploratory", "test-plan", "bug-report", "acceptance-test",
})
_QA_TESTING_PHRASES = (
    'test plan', 'write test cases', 'regression suite', 'bug report',
)

_MOBILE_QA_WORDS = frozenset({
    "mobile-qa", "device-matrix", "espresso", "xcuitest", "appium", "crashlytics", "beta-test",
})
_MOBILE_QA_PHRASES = (
    'mobile test plan', 'device matrix', 'appium test', 'beta testing app',
)

_ACCESSIBILITY_ENG_WORDS = frozenset({
    "aria", "screen-reader-eng", "focus-trap", "keyboard-nav", "axe", "wcag-eng", "accessible-name",
})
_ACCESSIBILITY_ENG_PHRASES = (
    'aria labels', 'keyboard navigation', 'axe audit', 'focus management',
)

_PERFORMANCE_WEB_WORDS = frozenset({
    "web-performance", "lcp", "cls", "inp", "lighthouse", "bundle-size", "lazy-load", "cdn",
})
_PERFORMANCE_WEB_PHRASES = (
    'improve lcp', 'core web vitals', 'lighthouse score', 'reduce bundle size',
)

_SEO_TECHNICAL_WORDS = frozenset({
    "technical-seo", "crawl", "indexability", "canonical", "schema-org", "robots-txt", "sitemap-xml", "core-web",
})
_SEO_TECHNICAL_PHRASES = (
    'technical seo audit', 'robots txt', 'schema markup', 'canonical tags',
)

_TAX_PREP_WORDS = frozenset({
    "tax-prep", "filing", "deduction", "w2", "1099", "schedule-c", "extension", "organizer",
})
_TAX_PREP_PHRASES = (
    'tax prep checklist', 'organize tax docs', 'schedule c basics', 'filing checklist',
)

_INSURANCE_CLAIMS_WORDS = frozenset({
    "claim", "claims", "adjuster", "deductible-claim", "fnol", "settlement", "appraisal",
})
_INSURANCE_CLAIMS_PHRASES = (
    'file a claim', 'insurance claim process', 'talk to adjuster', 'claim documentation',
)

_CAR_BUYING_WORDS = frozenset({
    "car-buying", "otr", "out-the-door", "apr", "trade-in", "cpo", "msrp", "dealer",
})
_CAR_BUYING_PHRASES = (
    'buy a car', 'out the door price', 'negotiate car', 'cpo vs new',
)

_HOME_BUYING_WORDS = frozenset({
    "home-buying", "offer", "inspection", "closing", "earnest", "contingency", "preapproval", "title",
})
_HOME_BUYING_PHRASES = (
    'make an offer', 'home inspection', 'closing checklist', 'earnest money',
)

_RENTING_WORDS = frozenset({
    "renting", "lease-signing", "roommate", "security-deposit", "sublet", "renter", "apartment-hunt",
})
_RENTING_PHRASES = (
    'sign a lease', 'roommate agreement', 'security deposit', 'apartment hunt',
)

_COLLEGE_APPS_WORDS = frozenset({
    "college-app", "common-app", "personal-statement", "supplements", "recommendation", "early-decision", "fafsa-app",
})
_COLLEGE_APPS_PHRASES = (
    'college essay', 'common app', 'school list', 'recommendation letter request',
)

_SCHOLARSHIPS_WORDS = frozenset({
    "scholarship", "scholarships", "merit-aid", "essay-scholarship", "deadline", "award",
})
_SCHOLARSHIPS_PHRASES = (
    'scholarship essay', 'find scholarships', 'merit aid', 'scholarship application',
)

_STUDY_ABROAD_WORDS = frozenset({
    "study-abroad", "semester-abroad", "exchange", "host-family", "visa-student", "program-fee",
})
_STUDY_ABROAD_PHRASES = (
    'study abroad program', 'semester abroad', 'study abroad packing', 'exchange student',
)

_INTERNSHIP_WORDS = frozenset({
    "internship", "intern", "co-op", "summer-intern", "return-offer", "informational-interview",
})
_INTERNSHIP_PHRASES = (
    'find internship', 'internship application', 'intern projects', 'return offer',
)

_CAREER_CHANGE_WORDS = frozenset({
    "career-change", "career-pivot", "transferable", "reskilling", "mid-career", "switch-fields",
})
_CAREER_CHANGE_PHRASES = (
    'career change plan', 'career pivot', 'transferable skills', 'switch careers',
)

_LINKEDIN_WORDS = frozenset({
    # Keep LinkedIn-specific tokens only — generic "messages"/"reply" must not
    # steal WhatsApp, email, or plain inbox asks into LinkedIn mode.
    "linkedin", "headline", "about-section", "connection-request", "inmail", "open-to-work",
})
_LINKEDIN_PHRASES = (
    'linkedin profile', 'linkedin headline', 'connection request', 'linkedin about',
    'linkedin messages', 'linkedin message', 'linkedin inbox', 'linkedin dm',
    'respond to my linkedin', 'reply to linkedin', 'linkedin reply',
    'answer linkedin messages', 'check linkedin messages', 'linkedin inmail',
)

_WHATSAPP_WORDS = frozenset({
    "whatsapp", "whats-app", "wa-web", "web-whatsapp",
})
_WHATSAPP_PHRASES = (
    'whatsapp messages', 'whatsapp message', 'whatsapp chat', 'whatsapp chats',
    'reply to whatsapp', 'reply to my whatsapp', 'respond to my whatsapp',
    'answer whatsapp', 'check whatsapp', 'whatsapp inbox', 'whats app',
    'open whatsapp', 'my whatsapp messages',
)

_NETWORKING_CAREER_WORDS = frozenset({
    "networking", "informational", "warm-intro", "coffee-chat", "follow-up", "relationship-capital",
})
_NETWORKING_CAREER_PHRASES = (
    'networking tips', 'informational interview', 'warm intro', 'coffee chat ask',
)

_HOA_LIVING_WORDS = frozenset({
    "hoa", "cc&rs", "ccrs", "hoa-board", "assessment", "violation", "architectural-review",
})
_HOA_LIVING_PHRASES = (
    'hoa rules', 'hoa dispute', 'architectural request', 'hoa meeting',
)

_COOP_HOUSING_WORDS = frozenset({
    "housing-coop", "co-op", "shareholder", "maintenance-fee", "board-coop", "proprietary-lease",
})
_COOP_HOUSING_PHRASES = (
    'housing co-op', 'buy into co-op', 'co-op board', 'maintenance fees',
)

_COMMUNITY_GARDEN_WORDS = frozenset({
    "community-garden", "allotment", "plot", "shared-garden", "garden-club", "tool-share",
})
_COMMUNITY_GARDEN_PHRASES = (
    'community garden plot', 'allotment tips', 'garden club', 'shared tools',
)

_MUTUAL_AID_WORDS = frozenset({
    "mutual-aid", "mutualaid", "community-care", "resource-share", "pod", "solidarity",
})
_MUTUAL_AID_PHRASES = (
    'mutual aid network', 'community care', 'resource share', 'solidarity fund',
)

_FISHKEEPING_WORDS = frozenset({
    "fishkeeping", "aquarium-hobby", "stocking", "water-change", "quarantine", "species-tank",
})
_FISHKEEPING_PHRASES = (
    'fish stocking', 'water change schedule', 'quarantine tank', 'species only tank',
)

_TERRARIUM_WORDS = frozenset({
    "terrarium", "vivarium", "closed-terrarium", "substrate-layers", "springtails", "bioactive",
})
_TERRARIUM_PHRASES = (
    'build terrarium', 'closed terrarium', 'bioactive vivarium', 'terrarium plants',
)

_ANTKEEPING_WORDS = frozenset({
    "antkeeping", "formicarium", "colony", "queen", "test-tube", "outworld", "ants",
})
_ANTKEEPING_PHRASES = (
    'ant colony', 'formicarium setup', 'test tube setup', 'antkeeping basics',
)

_BEEKEEPING_ADVANCED_WORDS = frozenset({
    "split-hive", "queen-rearing", "nuc", "varroa-treatment", "honey-flow", "overwinter",
})
_BEEKEEPING_ADVANCED_PHRASES = (
    'split a hive', 'queen rearing', 'varroa treatment', 'overwinter bees',
)

_FOUNTAIN_PEN_WORDS = frozenset({
    "fountain-pen", "nib", "ink", "converter", "piston", "flex-nib", "rhodia", "tomoe",
})
_FOUNTAIN_PEN_PHRASES = (
    'fountain pen ink', 'nib tuning', 'pen maintenance', 'ink recommendation',
)

_STATIONERY_WORDS = frozenset({
    "stationery", "notebook", "planner", "fountain-stationery", "paper", "pen-collection", "hobonichi",
})
_STATIONERY_PHRASES = (
    'notebook recommendation', 'planner setup', 'stationery haul', 'best paper',
)

_MECHANICAL_KEYBOARD_WORDS = frozenset({
    "mechanical-keyboard", "switches", "keycaps", "hotswap", "lubing", "stabilizers", "pcb", "gasket",
})
_MECHANICAL_KEYBOARD_PHRASES = (
    'keyboard build', 'switch recommendation', 'lube switches', 'keycap profile',
)

_PC_BUILDING_WORDS = frozenset({
    "pc-build", "pc-building", "motherboard", "cpu", "gpu", "psu", "case", "thermals", "cable-management",
})
_PC_BUILDING_PHRASES = (
    'pc build guide', 'choose a gpu', 'cable management', 'thermal paste',
)

_HOME_LAB_WORDS = frozenset({
    "homelab", "home-lab", "proxmox", "truenas", "self-hosted", "nas", "rack", "ups",
})
_HOME_LAB_PHRASES = (
    'homelab setup', 'proxmox server', 'self hosted services', 'nas setup',
)

_THREE_D_MODELING_WORDS = frozenset({
    "3d-modeling", "blender", "cad", "fusion360", "mesh", "topology", "sculpt", "stl",
})
_THREE_D_MODELING_PHRASES = (
    'blender modeling', 'fusion 360', 'retopology', 'model for 3d print',
)

_CNC_WORDS = frozenset({
    "cnc", "gcode", "toolpath", "endmill", "feeds", "speeds", "cam", "workholding",
})
_CNC_PHRASES = (
    'cnc toolpath', 'feeds and speeds', 'gcode basics', 'cnc workholding',
)

_LASER_CUTTING_WORDS = frozenset({
    "laser-cutting", "laser-cutter", "kerf", "vector-cut", "engraving", "acrylic", "lightburn",
})
_LASER_CUTTING_PHRASES = (
    'laser cut design', 'kerf adjustment', 'laser engraving', 'lightburn settings',
)

_RESIN_PRINTING_WORDS = frozenset({
    "resin", "sla", "dlp", "msla", "fep", "wash-cure", "supports-resin", "ipa",
})
_RESIN_PRINTING_PHRASES = (
    'resin print settings', 'wash and cure', 'fep replacement', 'resin safety',
)

_FILAMENT_PRINTING_WORDS = frozenset({
    "fdm", "filament", "nozzle", "bed-level", "retraction", "pla", "petg", "abs", "slicer",
})
_FILAMENT_PRINTING_PHRASES = (
    'fdm settings', 'bed leveling', 'retraction tuning', 'pla print',
)

_MEDITATION_WORDS = frozenset({
    "meditation", "mindfulness-med", "vipassana", "zazen", "guided", "sit", "cushion",
})
_MEDITATION_PHRASES = (
    'meditation practice', 'how to meditate', 'guided meditation', 'daily sit',
)

_STOICISM_WORDS = frozenset({
    "stoicism", "stoic", "marcus", "epictetus", "seneca", "dichotomy", "memento-mori", "amor-fati",
})
_STOICISM_PHRASES = (
    'stoic practice', 'dichotomy of control', 'stoic journal', 'marcus aurelius',
)

_JOURNAL_PROMPTS_WORDS = frozenset({
    "journal-prompt", "prompts", "reflection-prompt", "gratitude-prompt", "evening-pages",
})
_JOURNAL_PROMPTS_PHRASES = (
    'journal prompts', 'gratitude prompts', 'reflection questions', 'evening journal',
)

_HABIT_BUILDING_WORDS = frozenset({
    "habit", "habits", "streak", "cue", "routine", "atomic-habits", "implementation-intention",
})
_HABIT_BUILDING_PHRASES = (
    'build a habit', 'habit stack', 'break a habit', 'habit tracker',
)

_TIME_BLOCKING_WORDS = frozenset({
    "time-blocking", "timeblock", "calendar-block", "deep-work-block", "theme-days", "focus-block",
})
_TIME_BLOCKING_PHRASES = (
    'time blocking', 'theme days', 'block my calendar', 'deep work schedule',
)

_SECOND_BRAIN_WORDS = frozenset({
    "second-brain", "basb", "para-method", "code-notes", "progressive-summarization",
})
_SECOND_BRAIN_PHRASES = (
    'second brain setup', 'para method', 'basb', 'progressive summarization',
)

_PACKING_WORDS = frozenset({
    "packing", "packing-list", "carry-on", "capsule-travel", "toiletry", "liquids-bag",
})
_PACKING_PHRASES = (
    'packing list', 'carry on only', 'pack light', 'toiletry bag',
)

_TRAVEL_PHOTOGRAPHY_WORDS = frozenset({
    "travel-photography", "travel-photo", "street-travel", "golden-hour-travel", "compact-kit",
})
_TRAVEL_PHOTOGRAPHY_PHRASES = (
    'travel photography tips', 'pack camera kit', 'street travel photos', 'golden hour travel',
)

_SOLO_TRAVEL_WORDS = frozenset({
    "solo-travel", "traveling-alone", "solo-trip", "safety-solo", "solo-female", "hostels",
})
_SOLO_TRAVEL_PHRASES = (
    'solo travel tips', 'traveling alone', 'solo trip plan', 'hostel safety',
)

_FAMILY_TRAVEL_WORDS = frozenset({
    "family-travel", "kids-trip", "stroller", "road-trip-kids", "theme-park", "kid-friendly",
})
_FAMILY_TRAVEL_PHRASES = (
    'family trip plan', 'travel with kids', 'theme park day', 'road trip with kids',
)

_BUDGET_TRAVEL_WORDS = frozenset({
    "budget-travel", "cheap-flights", "hostel", "points-hacking", "slow-travel", "cost-per-day",
})
_BUDGET_TRAVEL_PHRASES = (
    'budget travel tips', 'cheap flights', 'travel on a budget', 'cost per day',
)

_POINTS_MILES_WORDS = frozenset({
    "points", "miles", "award-travel", "transfer-partners", "churning", "status", "loyalty",
})
_POINTS_MILES_PHRASES = (
    'award travel', 'transfer partners', 'points strategy', 'airline status',
)

_REAL_ESTATE_PHOTO_WORDS = frozenset({
    "real-estate-photo", "listing-photo", "hdr-bracket", "twilight-shot", "vacant-staging-photo",
})
_REAL_ESTATE_PHOTO_PHRASES = (
    'real estate photography', 'listing photos', 'hdr interiors', 'twilight exterior',
)

_STAGING_WORDS = frozenset({
    "staging", "home-staging", "declutter-stage", "furniture-placement", "vacant-staging", "show-ready",
})
_STAGING_PHRASES = (
    'stage a home', 'home staging tips', 'furniture placement', 'show ready',
)

_INTERIOR_STYLING_WORDS = frozenset({
    "styling", "vignette", "throw-pillows", "styling-shelves", "tablescape", "finish-layers",
})
_INTERIOR_STYLING_PHRASES = (
    'style a shelf', 'room vignette', 'tablescape ideas', 'styling tips',
)

_EVENT_PHOTOGRAPHY_WORDS = frozenset({
    "event-photography", "wedding-photo", "second-shooter", "candid", "reception", "flash-event",
})
_EVENT_PHOTOGRAPHY_PHRASES = (
    'wedding photography tips', 'event flash', 'candid shots', 'reception photos',
)

_PORTRAIT_PHOTO_WORDS = frozenset({
    "portrait", "portraiture", "posing", "headshot", "rembrandt", "softbox", "catchlight",
})
_PORTRAIT_PHOTO_PHRASES = (
    'portrait lighting', 'posing guide', 'headshot tips', 'softbox setup',
)

_STREET_PHOTO_WORDS = frozenset({
    "street-photography", "street-photo", "candid-street", "zone-focus", "decisive-moment",
})
_STREET_PHOTO_PHRASES = (
    'street photography tips', 'zone focusing', 'candid street', 'decisive moment',
)

_WILDLIFE_PHOTO_WORDS = frozenset({
    "wildlife-photography", "wildlife-photo", "telephoto", "hide", "fieldcraft", "bird-photo",
})
_WILDLIFE_PHOTO_PHRASES = (
    'wildlife photography', 'bird photography', 'telephoto technique', 'fieldcraft tips',
)

_ASTRO_IMAGING_PROC_WORDS = frozenset({
    "stacking-astro", "pixinsight", "siril", "stretch", "calibration-frames", "darks", "flats",
})
_ASTRO_IMAGING_PROC_PHRASES = (
    'stack astro images', 'siril workflow', 'pixinsight stretch', 'calibration frames',
)

_SELF_LABEL = re.compile(
    r"\b(?:i'?m|i am|as a|i'?m a)\s+"
    r"(?:an?\s+)?"
    r"(?P<role>"
    r"student|undergrad|graduate student|phd student|grad student|"
    r"professor|academic|researcher|scholar|teacher|educator|"
    r"writer|novelist|journalist|blogger|copywriter|"
    r"designer|product designer|ux designer|"
    r"founder|co-?founder|ceo|product manager|pm|"
    r"manager|executive|marketer|marketing|"
    r"salesperson|sales|ae|sdr|"
    r"parent|mom|dad|mother|father|"
    r"lawyer|attorney|"
    r"nurse|doctor|physician|clinician|"
    r"scientist|physicist|chemist|biologist|"
    r"support agent|customer success|"
    r"data (?:analyst|scientist)|analyst|"
    r"developer|engineer|programmer|sre|devops|"
    r"musician|filmmaker|photographer|artist|"
    r"realtor|real estate agent|"
    r"chef|cook|"
    r"gamer|game designer|"
    r"coach|athlete|"
    r"hr|people ops|"
    r"mechanical engineer|civil engineer|"
    r"security engineer|pentester|"
    r"farmer|"
    r"mechanic"
    r")\b",
    re.I,
)


def _score_code(raw: str, low: str, toks: set[str]) -> int:
    score = 0
    hits = toks & _CODE_WORDS
    score += min(len(hits), 6) * 2
    if _CODE_TOKEN_RE.search(raw):
        score += 3
    if _CODE_INTENT_RE.search(raw):
        score += 3
    if re.search(r"\b[A-Z][a-z]+(?:[A-Z][a-z0-9]+)+\b", raw):
        score += 1
    return score


# Never score these as domain keywords even if a lexicon mistakenly lists them.
# Short English glue words flip personas (e.g. “look at results” → backpacking
# because Appalachian Trail “AT” was stored as the token “at”).
_LEXICON_STOPWORDS = frozenset("""
a an the and or but if then than that this these those of in on at to from by
for with about into over after is are was were be been being do does did doing
have has had having i me my we us our you your he him his she her it its they
them their what which who whom whose when where why how can could should would
will shall may might must please just also only more some any all not no nor
so up out off on as per via vs etc look see get got go going went make made
find search result results help need want open close box core hundred family
school work life world home day time year people way man woman
""".split())


def _score_lexicon(
    low: str,
    toks: set[str],
    words: frozenset[str],
    phrases: tuple[str, ...],
) -> int:
    # Word hits: ignore stopwords and 1–2 letter tokens (except digits like 5k
    # are rare; prefer phrases for acronyms). Stops “at”/“will”/“who” disasters.
    matched = {
        t for t in (toks & words)
        if len(t) >= 3 and t not in _LEXICON_STOPWORDS
    }
    score = min(len(matched), 8) * 2
    for ph in phrases:
        if ph in low:
            score += 4
    return score


# Tie-break order when scores equal (lower index = preferred).
_PRIORITY = (
    Audience.CODE,
    Audience.SECURITY,
    Audience.LEGAL,
    Audience.HEALTH,
    Audience.MEDICINE,
    Audience.PHARMACY,
    Audience.DENTAL,
    Audience.VETERINARY,
    Audience.THERAPY,
    Audience.IMMIGRATION,
    Audience.ACCESSIBILITY,
    Audience.DISABILITY,
    Audience.NEURODIVERSITY,
    Audience.ACADEMIC,
    Audience.SCIENCE,
    Audience.ENGINEERING,
    Audience.DATA,
    Audience.ML_AI,
    Audience.SRE,
    Audience.DEVOPS,
    Audience.CLOUD,
    Audience.SYSTEM_DESIGN,
    Audience.OBSERVABILITY,
    Audience.PLATFORM_ENG,
    Audience.DATABASE,
    Audience.NETWORKING,
    Audience.MOBILE,
    Audience.WEBDEV,
    Audience.EMBEDDED,
    Audience.IOT,
    Audience.ARVR,
    Audience.GAME_DEV,
    Audience.FOUNDER,
    Audience.PRODUCT,
    Audience.VC,
    Audience.FINANCE,
    Audience.INVESTING,
    Audience.QUANT,
    Audience.CRYPTO,
    Audience.TAX,
    Audience.ACCOUNTING,
    Audience.SALES,
    Audience.MARKETING,
    Audience.GROWTH,
    Audience.SEO,
    Audience.BRAND,
    Audience.PRODUCT_MARKETING,
    Audience.COPYWRITING,
    Audience.CONTENT_CREATOR,
    Audience.BUSINESS,
    Audience.HR,
    Audience.SUPPORT,
    Audience.OPERATIONS,
    Audience.COMPLIANCE,
    Audience.PROCUREMENT,
    Audience.QUALITY,
    Audience.PROJECT_MGMT,
    Audience.AGILE,
    Audience.OKRS,
    Audience.CHANGE_MGMT,
    Audience.MEETING,
    Audience.FACILITATION,
    Audience.DESIGN,
    Audience.UX_RESEARCH,
    Audience.MUSIC,
    Audience.GUITAR,
    Audience.PIANO,
    Audience.SINGING,
    Audience.DJ,
    Audience.CREATIVE,
    Audience.ANIMATION,
    Audience.POETRY,
    Audience.JOURNALISM,
    Audience.PR,
    Audience.MEDIA,
    Audience.JOB,
    Audience.TEACHER,
    Audience.STUDENT,
    Audience.HIGHER_ED,
    Audience.HOMESCHOOL,
    Audience.TEST_PREP,
    Audience.SPECIAL_ED,
    Audience.ESL,
    Audience.LANGUAGE,
    Audience.WRITER,
    Audience.TECH_WRITING,
    Audience.POLICY,
    Audience.NONPROFIT,
    Audience.GRANT_WRITING,
    Audience.BOARD_GOVERNANCE,
    Audience.CAMPAIGN,
    Audience.UNION,
    Audience.LOCAL_GOV,
    Audience.HOUSING,
    Audience.HUMANITARIAN,
    Audience.SOCIAL_WORK,
    Audience.FOOD_SECURITY,
    Audience.REAL_ESTATE,
    Audience.PROPERTY_MGMT,
    Audience.TRAVEL,
    Audience.DIGITAL_NOMAD,
    Audience.EXPAT,
    Audience.COOKING,
    Audience.BAKING,
    Audience.BBQ,
    Audience.BARTENDING,
    Audience.COCKTAILS,
    Audience.TEA,
    Audience.COFFEE,
    Audience.WINE,
    Audience.BEER,
    Audience.FERMENTATION,
    Audience.RESTAURANT,
    Audience.GAMING,
    Audience.TABLETOP,
    Audience.CHESS,
    Audience.ANIME,
    Audience.COMICS,
    Audience.SPORTS,
    Audience.FITNESS,
    Audience.YOGA,
    Audience.CLIMBING,
    Audience.GOLF,
    Audience.FISHING,
    Audience.SWIMMING,
    Audience.SKIING,
    Audience.RUNNING,
    Audience.CYCLING,
    Audience.MARTIAL_ARTS,
    Audience.POWERLIFTING,
    Audience.BODYBUILDING,
    Audience.CALISTHENICS,
    Audience.TRIATHLON,
    Audience.PARKOUR,
    Audience.SURFING,
    Audience.SKATEBOARDING,
    Audience.KAYAKING,
    Audience.ROWING,
    Audience.SCUBA,
    Audience.MOTORSPORTS,
    Audience.MOTORCYCLE,
    Audience.HOSPITALITY,
    Audience.EVENTS,
    Audience.WEDDING,
    Audience.FASHION,
    Audience.MAKEUP,
    Audience.HAIR,
    Audience.SKINCARE,
    Audience.DIY,
    Audience.PLUMBING,
    Audience.ELECTRICAL_TRADE,
    Audience.HVAC,
    Audience.WOODWORKING,
    Audience.METALWORKING,
    Audience.ELECTRONICS,
    Audience.PRINTING_3D,
    Audience.SEWING,
    Audience.KNITTING,
    Audience.CERAMICS,
    Audience.CALLIGRAPHY,
    Audience.JEWELRY,
    Audience.LEGO,
    Audience.ENVIRONMENT,
    Audience.ZERO_WASTE,
    Audience.COMPOSTING,
    Audience.PERMACULTURE,
    Audience.MYCOLOGY,
    Audience.FORAGING,
    Audience.BEEKEEPING,
    Audience.GARDENING,
    Audience.OUTDOORS,
    Audience.SURVIVAL,
    Audience.DISASTER_PREP,
    Audience.SPIRITUAL,
    Audience.SENIOR,
    Audience.PARENT,
    Audience.CHILDCARE,
    Audience.PREGNANCY,
    Audience.PETS,
    Audience.AQUARIUM,
    Audience.BIRDING,
    Audience.HORSES,
    Audience.AUTOMOTIVE,
    Audience.EV,
    Audience.AGRICULTURE,
    Audience.PHOTOGRAPHY,
    Audience.FILM,
    Audience.PODCAST,
    Audience.STREAMING,
    Audience.HOME_THEATER,
    Audience.AUDIO_HIFI,
    Audience.ARCHITECTURE,
    Audience.INTERIOR,
    Audience.URBAN_PLANNING,
    Audience.TINY_HOME,
    Audience.SMART_HOME,
    Audience.SOLAR_HOME,
    Audience.DRONE,
    Audience.AVIATION,
    Audience.SPACEFLIGHT,
    Audience.MARITIME,
    Audience.ENERGY,
    Audience.TELECOM,
    Audience.INSURANCE,
    Audience.RETAIL,
    Audience.ECOMMERCE,
    Audience.AMAZON_FBA,
    Audience.ETSY,
    Audience.AFFILIATE,
    Audience.FRANCHISE,
    Audience.LOGISTICS,
    Audience.MANUFACTURING,
    Audience.CONSTRUCTION,
    Audience.ROBOTICS,
    Audience.MATH,
    Audience.STATS,
    Audience.PHILOSOPHY,
    Audience.HISTORY,
    Audience.GEOGRAPHY,
    Audience.CHEMISTRY,
    Audience.BIOLOGY,
    Audience.PHYSICS,
    Audience.GENETICS,
    Audience.BIOTECH,
    Audience.GEOLOGY,
    Audience.OCEAN,
    Audience.ARCHAEOLOGY,
    Audience.LINGUISTICS,
    Audience.ASTRONOMY,
    Audience.WEATHER,
    Audience.PUBLIC_HEALTH,
    Audience.FIRST_AID,
    Audience.NURSING,
    Audience.PHYSICAL_THERAPY,
    Audience.OPTOMETRY,
    Audience.NUTRITION,
    Audience.SLEEP,
    Audience.LIBRARY,
    Audience.THEATER,
    Audience.DANCE,
    Audience.ACTING,
    Audience.COMEDY,
    Audience.IMPROV,
    Audience.MILITARY,
    Audience.FIRE,
    Audience.POLICE,
    Audience.HAM_RADIO,
    Audience.WATCHES,
    Audience.MOVING,
    Audience.DECLUTTER,
    Audience.OPEN_SOURCE,
    Audience.DOCUMENTATION_SITE,
    Audience.FREELANCE,
    Audience.CONSULTING,
    Audience.COACHING,
    Audience.SPEAKING,
    Audience.NEGOTIATION,
    Audience.PATENT,
    Audience.RELATIONSHIPS,
    Audience.DATING,
    Audience.PRODUCTIVITY,
    Audience.PKM,
    Audience.REMOTE_WORK,
    Audience.CROWDFUNDING,
    Audience.GENEALOGY,
    Audience.COLLECTING,
    Audience.TENNIS,
    Audience.BASKETBALL,
    Audience.SOCCER,
    Audience.BASEBALL,
    Audience.HOCKEY,
    Audience.VOLLEYBALL,
    Audience.BOXING,
    Audience.WRESTLING,
    Audience.FENCING,
    Audience.ARCHERY,
    Audience.SAILING,
    Audience.HIKING,
    Audience.CAMPING,
    Audience.BACKPACKING,
    Audience.CROSSFIT,
    Audience.PILATES,
    Audience.GYMNASTICS,
    Audience.ICE_SKATING,
    Audience.OLYMPIC_LIFTING,
    Audience.DRUMS,
    Audience.BASS,
    Audience.VIOLIN,
    Audience.MUSIC_PRODUCTION,
    Audience.SOUND_DESIGN,
    Audience.VOICEOVER,
    Audience.SCREENWRITING,
    Audience.NOVEL,
    Audience.BLOGGING,
    Audience.JOURNALING,
    Audience.TRANSLATION,
    Audience.SIGN_LANGUAGE,
    Audience.CROCHET,
    Audience.EMBROIDERY,
    Audience.QUILTING,
    Audience.COSPLAY,
    Audience.MAGIC_TRICKS,
    Audience.MODEL_BUILDING,
    Audience.LANDSCAPING,
    Audience.ROOFING,
    Audience.PAINTING_TRADE,
    Audience.FLOORING,
    Audience.CARPENTRY,
    Audience.APPLIANCE_REPAIR,
    Audience.PEST_CONTROL,
    Audience.AUTO_BODY,
    Audience.DOG_TRAINING,
    Audience.CAT_CARE,
    Audience.CHICKENS,
    Audience.REPTILES,
    Audience.CAREGIVING,
    Audience.CHRONIC_ILLNESS,
    Audience.MASSAGE,
    Audience.MENTAL_FITNESS,
    Audience.FERTILITY,
    Audience.LACTATION,
    Audience.PSYCHOLOGY,
    Audience.NEUROSCIENCE,
    Audience.ECONOMICS,
    Audience.SOCIOLOGY,
    Audience.ANTHROPOLOGY,
    Audience.MATERIALS_SCIENCE,
    Audience.ECOLOGY,
    Audience.PERSONAL_FINANCE,
    Audience.RETIREMENT,
    Audience.ESTATE_PLANNING,
    Audience.SIDE_HUSTLE,
    Audience.REAL_ESTATE_INVESTING,
    Audience.IMPORT_EXPORT,
    Audience.INVENTORY,
    Audience.DATA_ENGINEERING,
    Audience.SPREADSHEETS,
    Audience.NOCODE,
    Audience.WORDPRESS,
    Audience.PRIVACY,
    Audience.PROMPT_ENG,
    Audience.KUBERNETES,
    Audience.GRAPHICS_PROG,
    Audience.COMPILER,
    Audience.API_DESIGN,
    Audience.FRONTEND,
    Audience.BACKEND,
    Audience.PARENTING_TEENS,
    Audience.ADOPTION,
    Audience.DIVORCE,
    Audience.GRIEF,
    Audience.MINIMALISM,
    Audience.LUXURY,
    Audience.THRIFTING,
    Audience.ROAD_TRIP,
    Audience.CRUISE,
    Audience.FOOD_TRAVEL,
    Audience.TUTORING,
    Audience.CURRICULUM,
    Audience.EARLY_CHILDHOOD,
    Audience.MONTESSORI,
    Audience.EDTECH,
    Audience.NEIGHBORHOOD,
    Audience.VOLUNTEERING,
    Audience.FUNDRAISING_EVENTS,
    Audience.PHOTOGRAPHY_EDITING,
    Audience.VIDEO_EDITING,
    Audience.PODCAST_EDITING,
    Audience.NEWSLETTER,
    Audience.COMMUNITY_MGMT,
    Audience.CUSTOMER_SUCCESS,
    Audience.REVENUE_OPS,
    Audience.PEOPLE_OPS,
    Audience.OFFICE_ADMIN,
    Audience.RESEARCH_METHODS,
    Audience.STATISTICS_APPLIED,
    Audience.CLIMATE_ACTION,
    Audience.RECYCLING,
    Audience.WATER_CONSERVATION,
    Audience.HOME_SECURITY,
    Audience.CYBER_HYGIENE,
    Audience.PASSWORD_SECURITY,
    Audience.BROWSER_EXT,
    Audience.EMAIL_PRODUCTIVITY,
    Audience.NOTE_TAKING,
    Audience.SPEED_READING,
    Audience.DEBATE,
    Audience.PUBLIC_POLICY_ANALYSIS,
    Audience.MAPS_GIS,
    Audience.CARTOGRAPHY,
    Audience.ASTROPHOTOGRAPHY,
    Audience.METEOROLOGY_HOBBY,
    Audience.AMATEUR_ASTRONOMY,
    Audience.BOARD_GAMES,
    Audience.PUZZLES,
    Audience.RUBIKS,
    Audience.ORIGAMI,
    Audience.KNIFE_SKILLS,
    Audience.MEAL_PREP,
    Audience.KETO,
    Audience.VEGAN_COOKING,
    Audience.GLUTEN_FREE,
    Audience.SOUS_VIDE,
    Audience.SMOKING_MEAT,
    Audience.PICKLEBALL,
    Audience.BADMINTON,
    Audience.TABLE_TENNIS,
    Audience.RUGBY,
    Audience.CRICKET,
    Audience.SOFTBALL,
    Audience.LACROSSE,
    Audience.WATER_POLO,
    Audience.DIVING_SPORT,
    Audience.SYNCHRONIZED_SWIM,
    Audience.EQUESTRIAN_SPORT,
    Audience.ESPORTS,
    Audience.SPEEDRUNNING,
    Audience.YOGA_THERAPY,
    Audience.MOBILITY,
    Audience.BREATHWORK,
    Audience.SPANISH,
    Audience.FRENCH,
    Audience.GERMAN,
    Audience.JAPANESE,
    Audience.MANDARIN,
    Audience.KOREAN,
    Audience.ITALIAN,
    Audience.PORTUGUESE,
    Audience.ARABIC,
    Audience.HINDI,
    Audience.GREEK_LANG,
    Audience.LATIN,
    Audience.UKULELE,
    Audience.SAXOPHONE,
    Audience.TRUMPET,
    Audience.FLUTE,
    Audience.HARMONICA,
    Audience.BANJO,
    Audience.MUSIC_THEORY,
    Audience.GRAPHIC_DESIGN,
    Audience.ILLUSTRATION,
    Audience.UX_WRITING,
    Audience.STORYBOARD,
    Audience.COLOR_GRADING,
    Audience.LIGHTING_DESIGN,
    Audience.COSTUME_DESIGN,
    Audience.SET_DESIGN,
    Audience.PASTRY,
    Audience.BREAD,
    Audience.CHOCOLATE,
    Audience.CHEESE,
    Audience.CHARCUTERIE,
    Audience.PRESERVING,
    Audience.INDIAN_COOKING,
    Audience.CHINESE_COOKING,
    Audience.MEXICAN_COOKING,
    Audience.ITALIAN_COOKING,
    Audience.JAPANESE_COOKING,
    Audience.BBQ_SAUCES,
    Audience.COFFEE_ROASTING,
    Audience.LATTE_ART,
    Audience.HOUSEPLANTS,
    Audience.HYDROPONICS,
    Audience.BONSAI,
    Audience.AQUAPONICS,
    Audience.LAWN_CARE,
    Audience.IRRIGATION,
    Audience.POOL_CARE,
    Audience.FIREPLACE,
    Audience.DENTAL_HYGIENE,
    Audience.PHARMACOLOGY,
    Audience.RADIOLOGY_LITERACY,
    Audience.NUTRITION_SCIENCE,
    Audience.EPIDEMIOLOGY,
    Audience.BIOSTATISTICS,
    Audience.BOOKKEEPING,
    Audience.PAYROLL,
    Audience.BILLING,
    Audience.PRICING,
    Audience.SALES_ENABLEMENT,
    Audience.PARTNERSHIPS,
    Audience.CUSTOMER_RESEARCH,
    Audience.ANALYTICS,
    Audience.AB_TESTING,
    Audience.RUST_LANG,
    Audience.GO_LANG,
    Audience.PYTHON_DATA,
    Audience.SQL_ANALYTICS,
    Audience.TERRAFORM,
    Audience.ANSIBLE,
    Audience.CICD,
    Audience.DOCKER,
    Audience.LINUX_ADMIN,
    Audience.NETWORK_SECURITY,
    Audience.PENTEST_DEFENSE,
    Audience.THREAT_MODEL,
    Audience.INCIDENT_RESPONSE,
    Audience.QA_TESTING,
    Audience.MOBILE_QA,
    Audience.ACCESSIBILITY_ENG,
    Audience.PERFORMANCE_WEB,
    Audience.SEO_TECHNICAL,
    Audience.TAX_PREP,
    Audience.INSURANCE_CLAIMS,
    Audience.CAR_BUYING,
    Audience.HOME_BUYING,
    Audience.RENTING,
    Audience.COLLEGE_APPS,
    Audience.SCHOLARSHIPS,
    Audience.STUDY_ABROAD,
    Audience.INTERNSHIP,
    Audience.CAREER_CHANGE,
    Audience.LINKEDIN,
    Audience.WHATSAPP,
    Audience.NETWORKING_CAREER,
    Audience.HOA_LIVING,
    Audience.COOP_HOUSING,
    Audience.COMMUNITY_GARDEN,
    Audience.MUTUAL_AID,
    Audience.FISHKEEPING,
    Audience.TERRARIUM,
    Audience.ANTKEEPING,
    Audience.BEEKEEPING_ADVANCED,
    Audience.FOUNTAIN_PEN,
    Audience.STATIONERY,
    Audience.MECHANICAL_KEYBOARD,
    Audience.PC_BUILDING,
    Audience.HOME_LAB,
    Audience.THREE_D_MODELING,
    Audience.CNC,
    Audience.LASER_CUTTING,
    Audience.RESIN_PRINTING,
    Audience.FILAMENT_PRINTING,
    Audience.MEDITATION,
    Audience.STOICISM,
    Audience.JOURNAL_PROMPTS,
    Audience.HABIT_BUILDING,
    Audience.TIME_BLOCKING,
    Audience.SECOND_BRAIN,
    Audience.PACKING,
    Audience.TRAVEL_PHOTOGRAPHY,
    Audience.SOLO_TRAVEL,
    Audience.FAMILY_TRAVEL,
    Audience.BUDGET_TRAVEL,
    Audience.POINTS_MILES,
    Audience.REAL_ESTATE_PHOTO,
    Audience.STAGING,
    Audience.INTERIOR_STYLING,
    Audience.EVENT_PHOTOGRAPHY,
    Audience.PORTRAIT_PHOTO,
    Audience.STREET_PHOTO,
    Audience.WILDLIFE_PHOTO,
    Audience.ASTRO_IMAGING_PROC,
    Audience.PLAIN,
)

_LEXICONS: list[tuple[Audience, frozenset[str], tuple[str, ...]]] = [
    (Audience.ACADEMIC, _ACADEMIC_WORDS, _ACADEMIC_PHRASES),
    (Audience.STUDENT, _STUDENT_WORDS, _STUDENT_PHRASES),
    (Audience.WRITER, _WRITER_WORDS, _WRITER_PHRASES),
    (Audience.BUSINESS, _BUSINESS_WORDS, _BUSINESS_PHRASES),
    (Audience.DESIGN, _DESIGN_WORDS, _DESIGN_PHRASES),
    (Audience.JOB, _JOB_WORDS, _JOB_PHRASES),
    (Audience.TEACHER, _TEACHER_WORDS, _TEACHER_PHRASES),
    (Audience.DATA, _DATA_WORDS, _DATA_PHRASES),
    (Audience.FOUNDER, _FOUNDER_WORDS, _FOUNDER_PHRASES),
    (Audience.LEGAL, _LEGAL_WORDS, _LEGAL_PHRASES),
    (Audience.PARENT, _PARENT_WORDS, _PARENT_PHRASES),
    (Audience.MARKETING, _MARKETING_WORDS, _MARKETING_PHRASES),
    (Audience.SALES, _SALES_WORDS, _SALES_PHRASES),
    (Audience.FINANCE, _FINANCE_WORDS, _FINANCE_PHRASES),
    (Audience.PRODUCT, _PRODUCT_WORDS, _PRODUCT_PHRASES),
    (Audience.SUPPORT, _SUPPORT_WORDS, _SUPPORT_PHRASES),
    (Audience.SCIENCE, _SCIENCE_WORDS, _SCIENCE_PHRASES),
    (Audience.LANGUAGE, _LANGUAGE_WORDS, _LANGUAGE_PHRASES),
    (Audience.CREATIVE, _CREATIVE_WORDS, _CREATIVE_PHRASES),
    (Audience.HEALTH, _HEALTH_WORDS, _HEALTH_PHRASES),
    (Audience.NONPROFIT, _NONPROFIT_WORDS, _NONPROFIT_PHRASES),
    (Audience.POLICY, _POLICY_WORDS, _POLICY_PHRASES),
    (Audience.REAL_ESTATE, _REAL_ESTATE_WORDS, _REAL_ESTATE_PHRASES),
    (Audience.TRAVEL, _TRAVEL_WORDS, _TRAVEL_PHRASES),
    (Audience.COOKING, _COOKING_WORDS, _COOKING_PHRASES),
    (Audience.GAMING, _GAMING_WORDS, _GAMING_PHRASES),
    (Audience.SPORTS, _SPORTS_WORDS, _SPORTS_PHRASES),
    (Audience.HR, _HR_WORDS, _HR_PHRASES),
    (Audience.JOURNALISM, _JOURNALISM_WORDS, _JOURNALISM_PHRASES),
    (Audience.ACCESSIBILITY, _ACCESSIBILITY_WORDS, _ACCESSIBILITY_PHRASES),
    (Audience.ENGINEERING, _ENGINEERING_WORDS, _ENGINEERING_PHRASES),
    (Audience.SECURITY, _SECURITY_WORDS, _SECURITY_PHRASES),
    (Audience.HOSPITALITY, _HOSPITALITY_WORDS, _HOSPITALITY_PHRASES),
    (Audience.EVENTS, _EVENTS_WORDS, _EVENTS_PHRASES),
    (Audience.FASHION, _FASHION_WORDS, _FASHION_PHRASES),
    (Audience.DIY, _DIY_WORDS, _DIY_PHRASES),
    (Audience.ENVIRONMENT, _ENVIRONMENT_WORDS, _ENVIRONMENT_PHRASES),
    (Audience.SPIRITUAL, _SPIRITUAL_WORDS, _SPIRITUAL_PHRASES),
    (Audience.SENIOR, _SENIOR_WORDS, _SENIOR_PHRASES),
    (Audience.AUTOMOTIVE, _AUTOMOTIVE_WORDS, _AUTOMOTIVE_PHRASES),
    (Audience.AGRICULTURE, _AGRICULTURE_WORDS, _AGRICULTURE_PHRASES),
    (Audience.MUSIC, _MUSIC_WORDS, _MUSIC_PHRASES),
    (Audience.PHOTOGRAPHY, _PHOTOGRAPHY_WORDS, _PHOTOGRAPHY_PHRASES),
    (Audience.FILM, _FILM_WORDS, _FILM_PHRASES),
    (Audience.PODCAST, _PODCAST_WORDS, _PODCAST_PHRASES),
    (Audience.ARCHITECTURE, _ARCHITECTURE_WORDS, _ARCHITECTURE_PHRASES),
    (Audience.INTERIOR, _INTERIOR_WORDS, _INTERIOR_PHRASES),
    (Audience.INSURANCE, _INSURANCE_WORDS, _INSURANCE_PHRASES),
    (Audience.TAX, _TAX_WORDS, _TAX_PHRASES),
    (Audience.INVESTING, _INVESTING_WORDS, _INVESTING_PHRASES),
    (Audience.CRYPTO, _CRYPTO_WORDS, _CRYPTO_PHRASES),
    (Audience.RETAIL, _RETAIL_WORDS, _RETAIL_PHRASES),
    (Audience.ECOMMERCE, _ECOMMERCE_WORDS, _ECOMMERCE_PHRASES),
    (Audience.LOGISTICS, _LOGISTICS_WORDS, _LOGISTICS_PHRASES),
    (Audience.MANUFACTURING, _MANUFACTURING_WORDS, _MANUFACTURING_PHRASES),
    (Audience.CONSTRUCTION, _CONSTRUCTION_WORDS, _CONSTRUCTION_PHRASES),
    (Audience.ROBOTICS, _ROBOTICS_WORDS, _ROBOTICS_PHRASES),
    (Audience.MATH, _MATH_WORDS, _MATH_PHRASES),
    (Audience.PHILOSOPHY, _PHILOSOPHY_WORDS, _PHILOSOPHY_PHRASES),
    (Audience.PETS, _PETS_WORDS, _PETS_PHRASES),
    (Audience.CHILDCARE, _CHILDCARE_WORDS, _CHILDCARE_PHRASES),
    (Audience.IMMIGRATION, _IMMIGRATION_WORDS, _IMMIGRATION_PHRASES),
    (Audience.THERAPY, _THERAPY_WORDS, _THERAPY_PHRASES),
    (Audience.LIBRARY, _LIBRARY_WORDS, _LIBRARY_PHRASES),
    (Audience.THEATER, _THEATER_WORDS, _THEATER_PHRASES),
    (Audience.DANCE, _DANCE_WORDS, _DANCE_PHRASES),
    (Audience.WEATHER, _WEATHER_WORDS, _WEATHER_PHRASES),
    (Audience.ASTRONOMY, _ASTRONOMY_WORDS, _ASTRONOMY_PHRASES),
    (Audience.COMPLIANCE, _COMPLIANCE_WORDS, _COMPLIANCE_PHRASES),
    (Audience.OPERATIONS, _OPERATIONS_WORDS, _OPERATIONS_PHRASES),
    (Audience.PROCUREMENT, _PROCUREMENT_WORDS, _PROCUREMENT_PHRASES),
    (Audience.QUALITY, _QUALITY_WORDS, _QUALITY_PHRASES),
    (Audience.GROWTH, _GROWTH_WORDS, _GROWTH_PHRASES),
    (Audience.UX_RESEARCH, _UX_RESEARCH_WORDS, _UX_RESEARCH_PHRASES),
    (Audience.STATS, _STATS_WORDS, _STATS_PHRASES),
    (Audience.GENEALOGY, _GENEALOGY_WORDS, _GENEALOGY_PHRASES),
    (Audience.COLLECTING, _COLLECTING_WORDS, _COLLECTING_PHRASES),
    (Audience.OUTDOORS, _OUTDOORS_WORDS, _OUTDOORS_PHRASES),
    (Audience.GARDENING, _GARDENING_WORDS, _GARDENING_PHRASES),
    (Audience.BAKING, _BAKING_WORDS, _BAKING_PHRASES),
    (Audience.COFFEE, _COFFEE_WORDS, _COFFEE_PHRASES),
    (Audience.WINE, _WINE_WORDS, _WINE_PHRASES),
    (Audience.BEER, _BEER_WORDS, _BEER_PHRASES),
    (Audience.AVIATION, _AVIATION_WORDS, _AVIATION_PHRASES),
    (Audience.MARITIME, _MARITIME_WORDS, _MARITIME_PHRASES),
    (Audience.ENERGY, _ENERGY_WORDS, _ENERGY_PHRASES),
    (Audience.TELECOM, _TELECOM_WORDS, _TELECOM_PHRASES),
    (Audience.MEDIA, _MEDIA_WORDS, _MEDIA_PHRASES),
    (Audience.PR, _PR_WORDS, _PR_PHRASES),
    (Audience.SOCIAL_WORK, _SOCIAL_WORK_WORDS, _SOCIAL_WORK_PHRASES),
    (Audience.ACCOUNTING, _ACCOUNTING_WORDS, _ACCOUNTING_PHRASES),
    (Audience.ACTING, _ACTING_WORDS, _ACTING_PHRASES),
    (Audience.COMEDY, _COMEDY_WORDS, _COMEDY_PHRASES),
    (Audience.WOODWORKING, _WOODWORKING_WORDS, _WOODWORKING_PHRASES),
    (Audience.METALWORKING, _METALWORKING_WORDS, _METALWORKING_PHRASES),
    (Audience.ELECTRONICS, _ELECTRONICS_WORDS, _ELECTRONICS_PHRASES),
    (Audience.PRINTING_3D, _PRINTING_3D_WORDS, _PRINTING_3D_PHRASES),
    (Audience.SEWING, _SEWING_WORDS, _SEWING_PHRASES),
    (Audience.KNITTING, _KNITTING_WORDS, _KNITTING_PHRASES),
    (Audience.CHESS, _CHESS_WORDS, _CHESS_PHRASES),
    (Audience.TABLETOP, _TABLETOP_WORDS, _TABLETOP_PHRASES),
    (Audience.ANIME, _ANIME_WORDS, _ANIME_PHRASES),
    (Audience.COMICS, _COMICS_WORDS, _COMICS_PHRASES),
    (Audience.SCUBA, _SCUBA_WORDS, _SCUBA_PHRASES),
    (Audience.CYCLING, _CYCLING_WORDS, _CYCLING_PHRASES),
    (Audience.RUNNING, _RUNNING_WORDS, _RUNNING_PHRASES),
    (Audience.MARTIAL_ARTS, _MARTIAL_ARTS_WORDS, _MARTIAL_ARTS_PHRASES),
    (Audience.NUTRITION, _NUTRITION_WORDS, _NUTRITION_PHRASES),
    (Audience.PRODUCTIVITY, _PRODUCTIVITY_WORDS, _PRODUCTIVITY_PHRASES),
    (Audience.PKM, _PKM_WORDS, _PKM_PHRASES),
    (Audience.DEVOPS, _DEVOPS_WORDS, _DEVOPS_PHRASES),
    (Audience.CLOUD, _CLOUD_WORDS, _CLOUD_PHRASES),
    (Audience.NETWORKING, _NETWORKING_WORDS, _NETWORKING_PHRASES),
    (Audience.DATABASE, _DATABASE_WORDS, _DATABASE_PHRASES),
    (Audience.MOBILE, _MOBILE_WORDS, _MOBILE_PHRASES),
    (Audience.WEBDEV, _WEBDEV_WORDS, _WEBDEV_PHRASES),
    (Audience.EMBEDDED, _EMBEDDED_WORDS, _EMBEDDED_PHRASES),
    (Audience.IOT, _IOT_WORDS, _IOT_PHRASES),
    (Audience.ARVR, _ARVR_WORDS, _ARVR_PHRASES),
    (Audience.FREELANCE, _FREELANCE_WORDS, _FREELANCE_PHRASES),
    (Audience.CONSULTING, _CONSULTING_WORDS, _CONSULTING_PHRASES),
    (Audience.COACHING, _COACHING_WORDS, _COACHING_PHRASES),
    (Audience.SPEAKING, _SPEAKING_WORDS, _SPEAKING_PHRASES),
    (Audience.RELATIONSHIPS, _RELATIONSHIPS_WORDS, _RELATIONSHIPS_PHRASES),
    (Audience.DATING, _DATING_WORDS, _DATING_PHRASES),
    (Audience.HISTORY, _HISTORY_WORDS, _HISTORY_PHRASES),
    (Audience.GEOGRAPHY, _GEOGRAPHY_WORDS, _GEOGRAPHY_PHRASES),
    (Audience.CHEMISTRY, _CHEMISTRY_WORDS, _CHEMISTRY_PHRASES),
    (Audience.BIOLOGY, _BIOLOGY_WORDS, _BIOLOGY_PHRASES),
    (Audience.PHYSICS, _PHYSICS_WORDS, _PHYSICS_PHRASES),
    (Audience.MEDICINE, _MEDICINE_WORDS, _MEDICINE_PHRASES),
    (Audience.NURSING, _NURSING_WORDS, _NURSING_PHRASES),
    (Audience.PHARMACY, _PHARMACY_WORDS, _PHARMACY_PHRASES),
    (Audience.DENTAL, _DENTAL_WORDS, _DENTAL_PHRASES),
    (Audience.VETERINARY, _VETERINARY_WORDS, _VETERINARY_PHRASES),
    (Audience.MILITARY, _MILITARY_WORDS, _MILITARY_PHRASES),
    (Audience.FIRE, _FIRE_WORDS, _FIRE_PHRASES),
    (Audience.POLICE, _POLICE_WORDS, _POLICE_PHRASES),
    (Audience.GEOLOGY, _GEOLOGY_WORDS, _GEOLOGY_PHRASES),
    (Audience.OCEAN, _OCEAN_WORDS, _OCEAN_PHRASES),
    (Audience.ARCHAEOLOGY, _ARCHAEOLOGY_WORDS, _ARCHAEOLOGY_PHRASES),
    (Audience.LINGUISTICS, _LINGUISTICS_WORDS, _LINGUISTICS_PHRASES),
    (Audience.FITNESS, _FITNESS_WORDS, _FITNESS_PHRASES),
    (Audience.YOGA, _YOGA_WORDS, _YOGA_PHRASES),
    (Audience.CLIMBING, _CLIMBING_WORDS, _CLIMBING_PHRASES),
    (Audience.GOLF, _GOLF_WORDS, _GOLF_PHRASES),
    (Audience.FISHING, _FISHING_WORDS, _FISHING_PHRASES),
    (Audience.SWIMMING, _SWIMMING_WORDS, _SWIMMING_PHRASES),
    (Audience.SKIING, _SKIING_WORDS, _SKIING_PHRASES),
    (Audience.MOTORCYCLE, _MOTORCYCLE_WORDS, _MOTORCYCLE_PHRASES),
    (Audience.DRONE, _DRONE_WORDS, _DRONE_PHRASES),
    (Audience.GAME_DEV, _GAME_DEV_WORDS, _GAME_DEV_PHRASES),
    (Audience.ANIMATION, _ANIMATION_WORDS, _ANIMATION_PHRASES),
    (Audience.POETRY, _POETRY_WORDS, _POETRY_PHRASES),
    (Audience.MAKEUP, _MAKEUP_WORDS, _MAKEUP_PHRASES),
    (Audience.HAIR, _HAIR_WORDS, _HAIR_PHRASES),
    (Audience.SKINCARE, _SKINCARE_WORDS, _SKINCARE_PHRASES),
    (Audience.WEDDING, _WEDDING_WORDS, _WEDDING_PHRASES),
    (Audience.PREGNANCY, _PREGNANCY_WORDS, _PREGNANCY_PHRASES),
    (Audience.SLEEP, _SLEEP_WORDS, _SLEEP_PHRASES),
    (Audience.FIRST_AID, _FIRST_AID_WORDS, _FIRST_AID_PHRASES),
    (Audience.PUBLIC_HEALTH, _PUBLIC_HEALTH_WORDS, _PUBLIC_HEALTH_PHRASES),
    (Audience.ML_AI, _ML_AI_WORDS, _ML_AI_PHRASES),
    (Audience.SRE, _SRE_WORDS, _SRE_PHRASES),
    (Audience.SYSTEM_DESIGN, _SYSTEM_DESIGN_WORDS, _SYSTEM_DESIGN_PHRASES),
    (Audience.TECH_WRITING, _TECH_WRITING_WORDS, _TECH_WRITING_PHRASES),
    (Audience.PROJECT_MGMT, _PROJECT_MGMT_WORDS, _PROJECT_MGMT_PHRASES),
    (Audience.AGILE, _AGILE_WORDS, _AGILE_PHRASES),
    (Audience.REMOTE_WORK, _REMOTE_WORK_WORDS, _REMOTE_WORK_PHRASES),
    (Audience.CONTENT_CREATOR, _CONTENT_CREATOR_WORDS, _CONTENT_CREATOR_PHRASES),
    (Audience.SEO, _SEO_WORDS, _SEO_PHRASES),
    (Audience.BRAND, _BRAND_WORDS, _BRAND_PHRASES),
    (Audience.NEGOTIATION, _NEGOTIATION_WORDS, _NEGOTIATION_PHRASES),
    (Audience.PATENT, _PATENT_WORDS, _PATENT_PHRASES),
    (Audience.HOMESCHOOL, _HOMESCHOOL_WORDS, _HOMESCHOOL_PHRASES),
    (Audience.TEST_PREP, _TEST_PREP_WORDS, _TEST_PREP_PHRASES),
    (Audience.BARTENDING, _BARTENDING_WORDS, _BARTENDING_PHRASES),
    (Audience.TEA, _TEA_WORDS, _TEA_PHRASES),
    (Audience.BBQ, _BBQ_WORDS, _BBQ_PHRASES),
    (Audience.BEEKEEPING, _BEEKEEPING_WORDS, _BEEKEEPING_PHRASES),
    (Audience.AQUARIUM, _AQUARIUM_WORDS, _AQUARIUM_PHRASES),
    (Audience.BIRDING, _BIRDING_WORDS, _BIRDING_PHRASES),
    (Audience.HORSES, _HORSES_WORDS, _HORSES_PHRASES),
    (Audience.SURVIVAL, _SURVIVAL_WORDS, _SURVIVAL_PHRASES),
    (Audience.SMART_HOME, _SMART_HOME_WORDS, _SMART_HOME_PHRASES),
    (Audience.AUDIO_HIFI, _AUDIO_HIFI_WORDS, _AUDIO_HIFI_PHRASES),
    (Audience.WATCHES, _WATCHES_WORDS, _WATCHES_PHRASES),
    (Audience.JEWELRY, _JEWELRY_WORDS, _JEWELRY_PHRASES),
    (Audience.CERAMICS, _CERAMICS_WORDS, _CERAMICS_PHRASES),
    (Audience.CALLIGRAPHY, _CALLIGRAPHY_WORDS, _CALLIGRAPHY_PHRASES),
    (Audience.LEGO, _LEGO_WORDS, _LEGO_PHRASES),
    (Audience.HAM_RADIO, _HAM_RADIO_WORDS, _HAM_RADIO_PHRASES),
    (Audience.QUANT, _QUANT_WORDS, _QUANT_PHRASES),
    (Audience.FRANCHISE, _FRANCHISE_WORDS, _FRANCHISE_PHRASES),
    (Audience.RESTAURANT, _RESTAURANT_WORDS, _RESTAURANT_PHRASES),
    (Audience.PROPERTY_MGMT, _PROPERTY_MGMT_WORDS, _PROPERTY_MGMT_PHRASES),
    (Audience.PLUMBING, _PLUMBING_WORDS, _PLUMBING_PHRASES),
    (Audience.ELECTRICAL_TRADE, _ELECTRICAL_TRADE_WORDS, _ELECTRICAL_TRADE_PHRASES),
    (Audience.HVAC, _HVAC_WORDS, _HVAC_PHRASES),
    (Audience.MOVING, _MOVING_WORDS, _MOVING_PHRASES),
    (Audience.DECLUTTER, _DECLUTTER_WORDS, _DECLUTTER_PHRASES),
    (Audience.DIGITAL_NOMAD, _DIGITAL_NOMAD_WORDS, _DIGITAL_NOMAD_PHRASES),
    (Audience.EXPAT, _EXPAT_WORDS, _EXPAT_PHRASES),
    (Audience.NEURODIVERSITY, _NEURODIVERSITY_WORDS, _NEURODIVERSITY_PHRASES),
    (Audience.DISABILITY, _DISABILITY_WORDS, _DISABILITY_PHRASES),
    (Audience.PHYSICAL_THERAPY, _PHYSICAL_THERAPY_WORDS, _PHYSICAL_THERAPY_PHRASES),
    (Audience.OPTOMETRY, _OPTOMETRY_WORDS, _OPTOMETRY_PHRASES),
    (Audience.GENETICS, _GENETICS_WORDS, _GENETICS_PHRASES),
    (Audience.BIOTECH, _BIOTECH_WORDS, _BIOTECH_PHRASES),
    (Audience.SPACEFLIGHT, _SPACEFLIGHT_WORDS, _SPACEFLIGHT_PHRASES),
    (Audience.URBAN_PLANNING, _URBAN_PLANNING_WORDS, _URBAN_PLANNING_PHRASES),
    (Audience.DJ, _DJ_WORDS, _DJ_PHRASES),
    (Audience.GUITAR, _GUITAR_WORDS, _GUITAR_PHRASES),
    (Audience.PIANO, _PIANO_WORDS, _PIANO_PHRASES),
    (Audience.SINGING, _SINGING_WORDS, _SINGING_PHRASES),
    (Audience.IMPROV, _IMPROV_WORDS, _IMPROV_PHRASES),
    (Audience.FACILITATION, _FACILITATION_WORDS, _FACILITATION_PHRASES),
    (Audience.UNION, _UNION_WORDS, _UNION_PHRASES),
    (Audience.CAMPAIGN, _CAMPAIGN_WORDS, _CAMPAIGN_PHRASES),
    (Audience.COCKTAILS, _COCKTAILS_WORDS, _COCKTAILS_PHRASES),
    (Audience.FERMENTATION, _FERMENTATION_WORDS, _FERMENTATION_PHRASES),
    (Audience.FORAGING, _FORAGING_WORDS, _FORAGING_PHRASES),
    (Audience.MYCOLOGY, _MYCOLOGY_WORDS, _MYCOLOGY_PHRASES),
    (Audience.PERMACULTURE, _PERMACULTURE_WORDS, _PERMACULTURE_PHRASES),
    (Audience.TINY_HOME, _TINY_HOME_WORDS, _TINY_HOME_PHRASES),
    (Audience.HOME_THEATER, _HOME_THEATER_WORDS, _HOME_THEATER_PHRASES),
    (Audience.STREAMING, _STREAMING_WORDS, _STREAMING_PHRASES),
    (Audience.OPEN_SOURCE, _OPEN_SOURCE_WORDS, _OPEN_SOURCE_PHRASES),
    (Audience.DOCUMENTATION_SITE, _DOCUMENTATION_SITE_WORDS, _DOCUMENTATION_SITE_PHRASES),
    (Audience.OBSERVABILITY, _OBSERVABILITY_WORDS, _OBSERVABILITY_PHRASES),
    (Audience.PLATFORM_ENG, _PLATFORM_ENG_WORDS, _PLATFORM_ENG_PHRASES),
    (Audience.PRODUCT_MARKETING, _PRODUCT_MARKETING_WORDS, _PRODUCT_MARKETING_PHRASES),
    (Audience.COPYWRITING, _COPYWRITING_WORDS, _COPYWRITING_PHRASES),
    (Audience.AFFILIATE, _AFFILIATE_WORDS, _AFFILIATE_PHRASES),
    (Audience.AMAZON_FBA, _AMAZON_FBA_WORDS, _AMAZON_FBA_PHRASES),
    (Audience.ETSY, _ETSY_WORDS, _ETSY_PHRASES),
    (Audience.GRANT_WRITING, _GRANT_WRITING_WORDS, _GRANT_WRITING_PHRASES),
    (Audience.BOARD_GOVERNANCE, _BOARD_GOVERNANCE_WORDS, _BOARD_GOVERNANCE_PHRASES),
    (Audience.HIGHER_ED, _HIGHER_ED_WORDS, _HIGHER_ED_PHRASES),
    (Audience.SPECIAL_ED, _SPECIAL_ED_WORDS, _SPECIAL_ED_PHRASES),
    (Audience.ESL, _ESL_WORDS, _ESL_PHRASES),
    (Audience.MEETING, _MEETING_WORDS, _MEETING_PHRASES),
    (Audience.OKRS, _OKRS_WORDS, _OKRS_PHRASES),
    (Audience.CHANGE_MGMT, _CHANGE_MGMT_WORDS, _CHANGE_MGMT_PHRASES),
    (Audience.VC, _VC_WORDS, _VC_PHRASES),
    (Audience.CROWDFUNDING, _CROWDFUNDING_WORDS, _CROWDFUNDING_PHRASES),
    (Audience.DISASTER_PREP, _DISASTER_PREP_WORDS, _DISASTER_PREP_PHRASES),
    (Audience.HUMANITARIAN, _HUMANITARIAN_WORDS, _HUMANITARIAN_PHRASES),
    (Audience.LOCAL_GOV, _LOCAL_GOV_WORDS, _LOCAL_GOV_PHRASES),
    (Audience.HOUSING, _HOUSING_WORDS, _HOUSING_PHRASES),
    (Audience.FOOD_SECURITY, _FOOD_SECURITY_WORDS, _FOOD_SECURITY_PHRASES),
    (Audience.ZERO_WASTE, _ZERO_WASTE_WORDS, _ZERO_WASTE_PHRASES),
    (Audience.COMPOSTING, _COMPOSTING_WORDS, _COMPOSTING_PHRASES),
    (Audience.SOLAR_HOME, _SOLAR_HOME_WORDS, _SOLAR_HOME_PHRASES),
    (Audience.EV, _EV_WORDS, _EV_PHRASES),
    (Audience.MOTORSPORTS, _MOTORSPORTS_WORDS, _MOTORSPORTS_PHRASES),
    (Audience.SKATEBOARDING, _SKATEBOARDING_WORDS, _SKATEBOARDING_PHRASES),
    (Audience.SURFING, _SURFING_WORDS, _SURFING_PHRASES),
    (Audience.KAYAKING, _KAYAKING_WORDS, _KAYAKING_PHRASES),
    (Audience.ROWING, _ROWING_WORDS, _ROWING_PHRASES),
    (Audience.TRIATHLON, _TRIATHLON_WORDS, _TRIATHLON_PHRASES),
    (Audience.POWERLIFTING, _POWERLIFTING_WORDS, _POWERLIFTING_PHRASES),
    (Audience.BODYBUILDING, _BODYBUILDING_WORDS, _BODYBUILDING_PHRASES),
    (Audience.CALISTHENICS, _CALISTHENICS_WORDS, _CALISTHENICS_PHRASES),
    (Audience.PARKOUR, _PARKOUR_WORDS, _PARKOUR_PHRASES),
    (Audience.TENNIS, _TENNIS_WORDS, _TENNIS_PHRASES),
    (Audience.BASKETBALL, _BASKETBALL_WORDS, _BASKETBALL_PHRASES),
    (Audience.SOCCER, _SOCCER_WORDS, _SOCCER_PHRASES),
    (Audience.BASEBALL, _BASEBALL_WORDS, _BASEBALL_PHRASES),
    (Audience.HOCKEY, _HOCKEY_WORDS, _HOCKEY_PHRASES),
    (Audience.VOLLEYBALL, _VOLLEYBALL_WORDS, _VOLLEYBALL_PHRASES),
    (Audience.BOXING, _BOXING_WORDS, _BOXING_PHRASES),
    (Audience.WRESTLING, _WRESTLING_WORDS, _WRESTLING_PHRASES),
    (Audience.FENCING, _FENCING_WORDS, _FENCING_PHRASES),
    (Audience.ARCHERY, _ARCHERY_WORDS, _ARCHERY_PHRASES),
    (Audience.SAILING, _SAILING_WORDS, _SAILING_PHRASES),
    (Audience.HIKING, _HIKING_WORDS, _HIKING_PHRASES),
    (Audience.CAMPING, _CAMPING_WORDS, _CAMPING_PHRASES),
    (Audience.BACKPACKING, _BACKPACKING_WORDS, _BACKPACKING_PHRASES),
    (Audience.CROSSFIT, _CROSSFIT_WORDS, _CROSSFIT_PHRASES),
    (Audience.PILATES, _PILATES_WORDS, _PILATES_PHRASES),
    (Audience.GYMNASTICS, _GYMNASTICS_WORDS, _GYMNASTICS_PHRASES),
    (Audience.ICE_SKATING, _ICE_SKATING_WORDS, _ICE_SKATING_PHRASES),
    (Audience.OLYMPIC_LIFTING, _OLYMPIC_LIFTING_WORDS, _OLYMPIC_LIFTING_PHRASES),
    (Audience.DRUMS, _DRUMS_WORDS, _DRUMS_PHRASES),
    (Audience.BASS, _BASS_WORDS, _BASS_PHRASES),
    (Audience.VIOLIN, _VIOLIN_WORDS, _VIOLIN_PHRASES),
    (Audience.MUSIC_PRODUCTION, _MUSIC_PRODUCTION_WORDS, _MUSIC_PRODUCTION_PHRASES),
    (Audience.SOUND_DESIGN, _SOUND_DESIGN_WORDS, _SOUND_DESIGN_PHRASES),
    (Audience.VOICEOVER, _VOICEOVER_WORDS, _VOICEOVER_PHRASES),
    (Audience.SCREENWRITING, _SCREENWRITING_WORDS, _SCREENWRITING_PHRASES),
    (Audience.NOVEL, _NOVEL_WORDS, _NOVEL_PHRASES),
    (Audience.BLOGGING, _BLOGGING_WORDS, _BLOGGING_PHRASES),
    (Audience.JOURNALING, _JOURNALING_WORDS, _JOURNALING_PHRASES),
    (Audience.TRANSLATION, _TRANSLATION_WORDS, _TRANSLATION_PHRASES),
    (Audience.SIGN_LANGUAGE, _SIGN_LANGUAGE_WORDS, _SIGN_LANGUAGE_PHRASES),
    (Audience.CROCHET, _CROCHET_WORDS, _CROCHET_PHRASES),
    (Audience.EMBROIDERY, _EMBROIDERY_WORDS, _EMBROIDERY_PHRASES),
    (Audience.QUILTING, _QUILTING_WORDS, _QUILTING_PHRASES),
    (Audience.COSPLAY, _COSPLAY_WORDS, _COSPLAY_PHRASES),
    (Audience.MAGIC_TRICKS, _MAGIC_TRICKS_WORDS, _MAGIC_TRICKS_PHRASES),
    (Audience.MODEL_BUILDING, _MODEL_BUILDING_WORDS, _MODEL_BUILDING_PHRASES),
    (Audience.LANDSCAPING, _LANDSCAPING_WORDS, _LANDSCAPING_PHRASES),
    (Audience.ROOFING, _ROOFING_WORDS, _ROOFING_PHRASES),
    (Audience.PAINTING_TRADE, _PAINTING_TRADE_WORDS, _PAINTING_TRADE_PHRASES),
    (Audience.FLOORING, _FLOORING_WORDS, _FLOORING_PHRASES),
    (Audience.CARPENTRY, _CARPENTRY_WORDS, _CARPENTRY_PHRASES),
    (Audience.APPLIANCE_REPAIR, _APPLIANCE_REPAIR_WORDS, _APPLIANCE_REPAIR_PHRASES),
    (Audience.PEST_CONTROL, _PEST_CONTROL_WORDS, _PEST_CONTROL_PHRASES),
    (Audience.AUTO_BODY, _AUTO_BODY_WORDS, _AUTO_BODY_PHRASES),
    (Audience.DOG_TRAINING, _DOG_TRAINING_WORDS, _DOG_TRAINING_PHRASES),
    (Audience.CAT_CARE, _CAT_CARE_WORDS, _CAT_CARE_PHRASES),
    (Audience.CHICKENS, _CHICKENS_WORDS, _CHICKENS_PHRASES),
    (Audience.REPTILES, _REPTILES_WORDS, _REPTILES_PHRASES),
    (Audience.CAREGIVING, _CAREGIVING_WORDS, _CAREGIVING_PHRASES),
    (Audience.CHRONIC_ILLNESS, _CHRONIC_ILLNESS_WORDS, _CHRONIC_ILLNESS_PHRASES),
    (Audience.MASSAGE, _MASSAGE_WORDS, _MASSAGE_PHRASES),
    (Audience.MENTAL_FITNESS, _MENTAL_FITNESS_WORDS, _MENTAL_FITNESS_PHRASES),
    (Audience.FERTILITY, _FERTILITY_WORDS, _FERTILITY_PHRASES),
    (Audience.LACTATION, _LACTATION_WORDS, _LACTATION_PHRASES),
    (Audience.PSYCHOLOGY, _PSYCHOLOGY_WORDS, _PSYCHOLOGY_PHRASES),
    (Audience.NEUROSCIENCE, _NEUROSCIENCE_WORDS, _NEUROSCIENCE_PHRASES),
    (Audience.ECONOMICS, _ECONOMICS_WORDS, _ECONOMICS_PHRASES),
    (Audience.SOCIOLOGY, _SOCIOLOGY_WORDS, _SOCIOLOGY_PHRASES),
    (Audience.ANTHROPOLOGY, _ANTHROPOLOGY_WORDS, _ANTHROPOLOGY_PHRASES),
    (Audience.MATERIALS_SCIENCE, _MATERIALS_SCIENCE_WORDS, _MATERIALS_SCIENCE_PHRASES),
    (Audience.ECOLOGY, _ECOLOGY_WORDS, _ECOLOGY_PHRASES),
    (Audience.PERSONAL_FINANCE, _PERSONAL_FINANCE_WORDS, _PERSONAL_FINANCE_PHRASES),
    (Audience.RETIREMENT, _RETIREMENT_WORDS, _RETIREMENT_PHRASES),
    (Audience.ESTATE_PLANNING, _ESTATE_PLANNING_WORDS, _ESTATE_PLANNING_PHRASES),
    (Audience.SIDE_HUSTLE, _SIDE_HUSTLE_WORDS, _SIDE_HUSTLE_PHRASES),
    (Audience.REAL_ESTATE_INVESTING, _REAL_ESTATE_INVESTING_WORDS, _REAL_ESTATE_INVESTING_PHRASES),
    (Audience.IMPORT_EXPORT, _IMPORT_EXPORT_WORDS, _IMPORT_EXPORT_PHRASES),
    (Audience.INVENTORY, _INVENTORY_WORDS, _INVENTORY_PHRASES),
    (Audience.DATA_ENGINEERING, _DATA_ENGINEERING_WORDS, _DATA_ENGINEERING_PHRASES),
    (Audience.SPREADSHEETS, _SPREADSHEETS_WORDS, _SPREADSHEETS_PHRASES),
    (Audience.NOCODE, _NOCODE_WORDS, _NOCODE_PHRASES),
    (Audience.WORDPRESS, _WORDPRESS_WORDS, _WORDPRESS_PHRASES),
    (Audience.PRIVACY, _PRIVACY_WORDS, _PRIVACY_PHRASES),
    (Audience.PROMPT_ENG, _PROMPT_ENG_WORDS, _PROMPT_ENG_PHRASES),
    (Audience.KUBERNETES, _KUBERNETES_WORDS, _KUBERNETES_PHRASES),
    (Audience.GRAPHICS_PROG, _GRAPHICS_PROG_WORDS, _GRAPHICS_PROG_PHRASES),
    (Audience.COMPILER, _COMPILER_WORDS, _COMPILER_PHRASES),
    (Audience.API_DESIGN, _API_DESIGN_WORDS, _API_DESIGN_PHRASES),
    (Audience.FRONTEND, _FRONTEND_WORDS, _FRONTEND_PHRASES),
    (Audience.BACKEND, _BACKEND_WORDS, _BACKEND_PHRASES),
    (Audience.PARENTING_TEENS, _PARENTING_TEENS_WORDS, _PARENTING_TEENS_PHRASES),
    (Audience.ADOPTION, _ADOPTION_WORDS, _ADOPTION_PHRASES),
    (Audience.DIVORCE, _DIVORCE_WORDS, _DIVORCE_PHRASES),
    (Audience.GRIEF, _GRIEF_WORDS, _GRIEF_PHRASES),
    (Audience.MINIMALISM, _MINIMALISM_WORDS, _MINIMALISM_PHRASES),
    (Audience.LUXURY, _LUXURY_WORDS, _LUXURY_PHRASES),
    (Audience.THRIFTING, _THRIFTING_WORDS, _THRIFTING_PHRASES),
    (Audience.ROAD_TRIP, _ROAD_TRIP_WORDS, _ROAD_TRIP_PHRASES),
    (Audience.CRUISE, _CRUISE_WORDS, _CRUISE_PHRASES),
    (Audience.FOOD_TRAVEL, _FOOD_TRAVEL_WORDS, _FOOD_TRAVEL_PHRASES),
    (Audience.TUTORING, _TUTORING_WORDS, _TUTORING_PHRASES),
    (Audience.CURRICULUM, _CURRICULUM_WORDS, _CURRICULUM_PHRASES),
    (Audience.EARLY_CHILDHOOD, _EARLY_CHILDHOOD_WORDS, _EARLY_CHILDHOOD_PHRASES),
    (Audience.MONTESSORI, _MONTESSORI_WORDS, _MONTESSORI_PHRASES),
    (Audience.EDTECH, _EDTECH_WORDS, _EDTECH_PHRASES),
    (Audience.NEIGHBORHOOD, _NEIGHBORHOOD_WORDS, _NEIGHBORHOOD_PHRASES),
    (Audience.VOLUNTEERING, _VOLUNTEERING_WORDS, _VOLUNTEERING_PHRASES),
    (Audience.FUNDRAISING_EVENTS, _FUNDRAISING_EVENTS_WORDS, _FUNDRAISING_EVENTS_PHRASES),
    (Audience.PHOTOGRAPHY_EDITING, _PHOTOGRAPHY_EDITING_WORDS, _PHOTOGRAPHY_EDITING_PHRASES),
    (Audience.VIDEO_EDITING, _VIDEO_EDITING_WORDS, _VIDEO_EDITING_PHRASES),
    (Audience.PODCAST_EDITING, _PODCAST_EDITING_WORDS, _PODCAST_EDITING_PHRASES),
    (Audience.NEWSLETTER, _NEWSLETTER_WORDS, _NEWSLETTER_PHRASES),
    (Audience.COMMUNITY_MGMT, _COMMUNITY_MGMT_WORDS, _COMMUNITY_MGMT_PHRASES),
    (Audience.CUSTOMER_SUCCESS, _CUSTOMER_SUCCESS_WORDS, _CUSTOMER_SUCCESS_PHRASES),
    (Audience.REVENUE_OPS, _REVENUE_OPS_WORDS, _REVENUE_OPS_PHRASES),
    (Audience.PEOPLE_OPS, _PEOPLE_OPS_WORDS, _PEOPLE_OPS_PHRASES),
    (Audience.OFFICE_ADMIN, _OFFICE_ADMIN_WORDS, _OFFICE_ADMIN_PHRASES),
    (Audience.RESEARCH_METHODS, _RESEARCH_METHODS_WORDS, _RESEARCH_METHODS_PHRASES),
    (Audience.STATISTICS_APPLIED, _STATISTICS_APPLIED_WORDS, _STATISTICS_APPLIED_PHRASES),
    (Audience.CLIMATE_ACTION, _CLIMATE_ACTION_WORDS, _CLIMATE_ACTION_PHRASES),
    (Audience.RECYCLING, _RECYCLING_WORDS, _RECYCLING_PHRASES),
    (Audience.WATER_CONSERVATION, _WATER_CONSERVATION_WORDS, _WATER_CONSERVATION_PHRASES),
    (Audience.HOME_SECURITY, _HOME_SECURITY_WORDS, _HOME_SECURITY_PHRASES),
    (Audience.CYBER_HYGIENE, _CYBER_HYGIENE_WORDS, _CYBER_HYGIENE_PHRASES),
    (Audience.PASSWORD_SECURITY, _PASSWORD_SECURITY_WORDS, _PASSWORD_SECURITY_PHRASES),
    (Audience.BROWSER_EXT, _BROWSER_EXT_WORDS, _BROWSER_EXT_PHRASES),
    (Audience.EMAIL_PRODUCTIVITY, _EMAIL_PRODUCTIVITY_WORDS, _EMAIL_PRODUCTIVITY_PHRASES),
    (Audience.NOTE_TAKING, _NOTE_TAKING_WORDS, _NOTE_TAKING_PHRASES),
    (Audience.SPEED_READING, _SPEED_READING_WORDS, _SPEED_READING_PHRASES),
    (Audience.DEBATE, _DEBATE_WORDS, _DEBATE_PHRASES),
    (Audience.PUBLIC_POLICY_ANALYSIS, _PUBLIC_POLICY_ANALYSIS_WORDS, _PUBLIC_POLICY_ANALYSIS_PHRASES),
    (Audience.MAPS_GIS, _MAPS_GIS_WORDS, _MAPS_GIS_PHRASES),
    (Audience.CARTOGRAPHY, _CARTOGRAPHY_WORDS, _CARTOGRAPHY_PHRASES),
    (Audience.ASTROPHOTOGRAPHY, _ASTROPHOTOGRAPHY_WORDS, _ASTROPHOTOGRAPHY_PHRASES),
    (Audience.METEOROLOGY_HOBBY, _METEOROLOGY_HOBBY_WORDS, _METEOROLOGY_HOBBY_PHRASES),
    (Audience.AMATEUR_ASTRONOMY, _AMATEUR_ASTRONOMY_WORDS, _AMATEUR_ASTRONOMY_PHRASES),
    (Audience.BOARD_GAMES, _BOARD_GAMES_WORDS, _BOARD_GAMES_PHRASES),
    (Audience.PUZZLES, _PUZZLES_WORDS, _PUZZLES_PHRASES),
    (Audience.RUBIKS, _RUBIKS_WORDS, _RUBIKS_PHRASES),
    (Audience.ORIGAMI, _ORIGAMI_WORDS, _ORIGAMI_PHRASES),
    (Audience.KNIFE_SKILLS, _KNIFE_SKILLS_WORDS, _KNIFE_SKILLS_PHRASES),
    (Audience.MEAL_PREP, _MEAL_PREP_WORDS, _MEAL_PREP_PHRASES),
    (Audience.KETO, _KETO_WORDS, _KETO_PHRASES),
    (Audience.VEGAN_COOKING, _VEGAN_COOKING_WORDS, _VEGAN_COOKING_PHRASES),
    (Audience.GLUTEN_FREE, _GLUTEN_FREE_WORDS, _GLUTEN_FREE_PHRASES),
    (Audience.SOUS_VIDE, _SOUS_VIDE_WORDS, _SOUS_VIDE_PHRASES),
    (Audience.SMOKING_MEAT, _SMOKING_MEAT_WORDS, _SMOKING_MEAT_PHRASES),
    (Audience.PICKLEBALL, _PICKLEBALL_WORDS, _PICKLEBALL_PHRASES),
    (Audience.BADMINTON, _BADMINTON_WORDS, _BADMINTON_PHRASES),
    (Audience.TABLE_TENNIS, _TABLE_TENNIS_WORDS, _TABLE_TENNIS_PHRASES),
    (Audience.RUGBY, _RUGBY_WORDS, _RUGBY_PHRASES),
    (Audience.CRICKET, _CRICKET_WORDS, _CRICKET_PHRASES),
    (Audience.SOFTBALL, _SOFTBALL_WORDS, _SOFTBALL_PHRASES),
    (Audience.LACROSSE, _LACROSSE_WORDS, _LACROSSE_PHRASES),
    (Audience.WATER_POLO, _WATER_POLO_WORDS, _WATER_POLO_PHRASES),
    (Audience.DIVING_SPORT, _DIVING_SPORT_WORDS, _DIVING_SPORT_PHRASES),
    (Audience.SYNCHRONIZED_SWIM, _SYNCHRONIZED_SWIM_WORDS, _SYNCHRONIZED_SWIM_PHRASES),
    (Audience.EQUESTRIAN_SPORT, _EQUESTRIAN_SPORT_WORDS, _EQUESTRIAN_SPORT_PHRASES),
    (Audience.ESPORTS, _ESPORTS_WORDS, _ESPORTS_PHRASES),
    (Audience.SPEEDRUNNING, _SPEEDRUNNING_WORDS, _SPEEDRUNNING_PHRASES),
    (Audience.YOGA_THERAPY, _YOGA_THERAPY_WORDS, _YOGA_THERAPY_PHRASES),
    (Audience.MOBILITY, _MOBILITY_WORDS, _MOBILITY_PHRASES),
    (Audience.BREATHWORK, _BREATHWORK_WORDS, _BREATHWORK_PHRASES),
    (Audience.SPANISH, _SPANISH_WORDS, _SPANISH_PHRASES),
    (Audience.FRENCH, _FRENCH_WORDS, _FRENCH_PHRASES),
    (Audience.GERMAN, _GERMAN_WORDS, _GERMAN_PHRASES),
    (Audience.JAPANESE, _JAPANESE_WORDS, _JAPANESE_PHRASES),
    (Audience.MANDARIN, _MANDARIN_WORDS, _MANDARIN_PHRASES),
    (Audience.KOREAN, _KOREAN_WORDS, _KOREAN_PHRASES),
    (Audience.ITALIAN, _ITALIAN_WORDS, _ITALIAN_PHRASES),
    (Audience.PORTUGUESE, _PORTUGUESE_WORDS, _PORTUGUESE_PHRASES),
    (Audience.ARABIC, _ARABIC_WORDS, _ARABIC_PHRASES),
    (Audience.HINDI, _HINDI_WORDS, _HINDI_PHRASES),
    (Audience.GREEK_LANG, _GREEK_LANG_WORDS, _GREEK_LANG_PHRASES),
    (Audience.LATIN, _LATIN_WORDS, _LATIN_PHRASES),
    (Audience.UKULELE, _UKULELE_WORDS, _UKULELE_PHRASES),
    (Audience.SAXOPHONE, _SAXOPHONE_WORDS, _SAXOPHONE_PHRASES),
    (Audience.TRUMPET, _TRUMPET_WORDS, _TRUMPET_PHRASES),
    (Audience.FLUTE, _FLUTE_WORDS, _FLUTE_PHRASES),
    (Audience.HARMONICA, _HARMONICA_WORDS, _HARMONICA_PHRASES),
    (Audience.BANJO, _BANJO_WORDS, _BANJO_PHRASES),
    (Audience.MUSIC_THEORY, _MUSIC_THEORY_WORDS, _MUSIC_THEORY_PHRASES),
    (Audience.GRAPHIC_DESIGN, _GRAPHIC_DESIGN_WORDS, _GRAPHIC_DESIGN_PHRASES),
    (Audience.ILLUSTRATION, _ILLUSTRATION_WORDS, _ILLUSTRATION_PHRASES),
    (Audience.UX_WRITING, _UX_WRITING_WORDS, _UX_WRITING_PHRASES),
    (Audience.STORYBOARD, _STORYBOARD_WORDS, _STORYBOARD_PHRASES),
    (Audience.COLOR_GRADING, _COLOR_GRADING_WORDS, _COLOR_GRADING_PHRASES),
    (Audience.LIGHTING_DESIGN, _LIGHTING_DESIGN_WORDS, _LIGHTING_DESIGN_PHRASES),
    (Audience.COSTUME_DESIGN, _COSTUME_DESIGN_WORDS, _COSTUME_DESIGN_PHRASES),
    (Audience.SET_DESIGN, _SET_DESIGN_WORDS, _SET_DESIGN_PHRASES),
    (Audience.PASTRY, _PASTRY_WORDS, _PASTRY_PHRASES),
    (Audience.BREAD, _BREAD_WORDS, _BREAD_PHRASES),
    (Audience.CHOCOLATE, _CHOCOLATE_WORDS, _CHOCOLATE_PHRASES),
    (Audience.CHEESE, _CHEESE_WORDS, _CHEESE_PHRASES),
    (Audience.CHARCUTERIE, _CHARCUTERIE_WORDS, _CHARCUTERIE_PHRASES),
    (Audience.PRESERVING, _PRESERVING_WORDS, _PRESERVING_PHRASES),
    (Audience.INDIAN_COOKING, _INDIAN_COOKING_WORDS, _INDIAN_COOKING_PHRASES),
    (Audience.CHINESE_COOKING, _CHINESE_COOKING_WORDS, _CHINESE_COOKING_PHRASES),
    (Audience.MEXICAN_COOKING, _MEXICAN_COOKING_WORDS, _MEXICAN_COOKING_PHRASES),
    (Audience.ITALIAN_COOKING, _ITALIAN_COOKING_WORDS, _ITALIAN_COOKING_PHRASES),
    (Audience.JAPANESE_COOKING, _JAPANESE_COOKING_WORDS, _JAPANESE_COOKING_PHRASES),
    (Audience.BBQ_SAUCES, _BBQ_SAUCES_WORDS, _BBQ_SAUCES_PHRASES),
    (Audience.COFFEE_ROASTING, _COFFEE_ROASTING_WORDS, _COFFEE_ROASTING_PHRASES),
    (Audience.LATTE_ART, _LATTE_ART_WORDS, _LATTE_ART_PHRASES),
    (Audience.HOUSEPLANTS, _HOUSEPLANTS_WORDS, _HOUSEPLANTS_PHRASES),
    (Audience.HYDROPONICS, _HYDROPONICS_WORDS, _HYDROPONICS_PHRASES),
    (Audience.BONSAI, _BONSAI_WORDS, _BONSAI_PHRASES),
    (Audience.AQUAPONICS, _AQUAPONICS_WORDS, _AQUAPONICS_PHRASES),
    (Audience.LAWN_CARE, _LAWN_CARE_WORDS, _LAWN_CARE_PHRASES),
    (Audience.IRRIGATION, _IRRIGATION_WORDS, _IRRIGATION_PHRASES),
    (Audience.POOL_CARE, _POOL_CARE_WORDS, _POOL_CARE_PHRASES),
    (Audience.FIREPLACE, _FIREPLACE_WORDS, _FIREPLACE_PHRASES),
    (Audience.DENTAL_HYGIENE, _DENTAL_HYGIENE_WORDS, _DENTAL_HYGIENE_PHRASES),
    (Audience.PHARMACOLOGY, _PHARMACOLOGY_WORDS, _PHARMACOLOGY_PHRASES),
    (Audience.RADIOLOGY_LITERACY, _RADIOLOGY_LITERACY_WORDS, _RADIOLOGY_LITERACY_PHRASES),
    (Audience.NUTRITION_SCIENCE, _NUTRITION_SCIENCE_WORDS, _NUTRITION_SCIENCE_PHRASES),
    (Audience.EPIDEMIOLOGY, _EPIDEMIOLOGY_WORDS, _EPIDEMIOLOGY_PHRASES),
    (Audience.BIOSTATISTICS, _BIOSTATISTICS_WORDS, _BIOSTATISTICS_PHRASES),
    (Audience.BOOKKEEPING, _BOOKKEEPING_WORDS, _BOOKKEEPING_PHRASES),
    (Audience.PAYROLL, _PAYROLL_WORDS, _PAYROLL_PHRASES),
    (Audience.BILLING, _BILLING_WORDS, _BILLING_PHRASES),
    (Audience.PRICING, _PRICING_WORDS, _PRICING_PHRASES),
    (Audience.SALES_ENABLEMENT, _SALES_ENABLEMENT_WORDS, _SALES_ENABLEMENT_PHRASES),
    (Audience.PARTNERSHIPS, _PARTNERSHIPS_WORDS, _PARTNERSHIPS_PHRASES),
    (Audience.CUSTOMER_RESEARCH, _CUSTOMER_RESEARCH_WORDS, _CUSTOMER_RESEARCH_PHRASES),
    (Audience.ANALYTICS, _ANALYTICS_WORDS, _ANALYTICS_PHRASES),
    (Audience.AB_TESTING, _AB_TESTING_WORDS, _AB_TESTING_PHRASES),
    (Audience.RUST_LANG, _RUST_LANG_WORDS, _RUST_LANG_PHRASES),
    (Audience.GO_LANG, _GO_LANG_WORDS, _GO_LANG_PHRASES),
    (Audience.PYTHON_DATA, _PYTHON_DATA_WORDS, _PYTHON_DATA_PHRASES),
    (Audience.SQL_ANALYTICS, _SQL_ANALYTICS_WORDS, _SQL_ANALYTICS_PHRASES),
    (Audience.TERRAFORM, _TERRAFORM_WORDS, _TERRAFORM_PHRASES),
    (Audience.ANSIBLE, _ANSIBLE_WORDS, _ANSIBLE_PHRASES),
    (Audience.CICD, _CICD_WORDS, _CICD_PHRASES),
    (Audience.DOCKER, _DOCKER_WORDS, _DOCKER_PHRASES),
    (Audience.LINUX_ADMIN, _LINUX_ADMIN_WORDS, _LINUX_ADMIN_PHRASES),
    (Audience.NETWORK_SECURITY, _NETWORK_SECURITY_WORDS, _NETWORK_SECURITY_PHRASES),
    (Audience.PENTEST_DEFENSE, _PENTEST_DEFENSE_WORDS, _PENTEST_DEFENSE_PHRASES),
    (Audience.THREAT_MODEL, _THREAT_MODEL_WORDS, _THREAT_MODEL_PHRASES),
    (Audience.INCIDENT_RESPONSE, _INCIDENT_RESPONSE_WORDS, _INCIDENT_RESPONSE_PHRASES),
    (Audience.QA_TESTING, _QA_TESTING_WORDS, _QA_TESTING_PHRASES),
    (Audience.MOBILE_QA, _MOBILE_QA_WORDS, _MOBILE_QA_PHRASES),
    (Audience.ACCESSIBILITY_ENG, _ACCESSIBILITY_ENG_WORDS, _ACCESSIBILITY_ENG_PHRASES),
    (Audience.PERFORMANCE_WEB, _PERFORMANCE_WEB_WORDS, _PERFORMANCE_WEB_PHRASES),
    (Audience.SEO_TECHNICAL, _SEO_TECHNICAL_WORDS, _SEO_TECHNICAL_PHRASES),
    (Audience.TAX_PREP, _TAX_PREP_WORDS, _TAX_PREP_PHRASES),
    (Audience.INSURANCE_CLAIMS, _INSURANCE_CLAIMS_WORDS, _INSURANCE_CLAIMS_PHRASES),
    (Audience.CAR_BUYING, _CAR_BUYING_WORDS, _CAR_BUYING_PHRASES),
    (Audience.HOME_BUYING, _HOME_BUYING_WORDS, _HOME_BUYING_PHRASES),
    (Audience.RENTING, _RENTING_WORDS, _RENTING_PHRASES),
    (Audience.COLLEGE_APPS, _COLLEGE_APPS_WORDS, _COLLEGE_APPS_PHRASES),
    (Audience.SCHOLARSHIPS, _SCHOLARSHIPS_WORDS, _SCHOLARSHIPS_PHRASES),
    (Audience.STUDY_ABROAD, _STUDY_ABROAD_WORDS, _STUDY_ABROAD_PHRASES),
    (Audience.INTERNSHIP, _INTERNSHIP_WORDS, _INTERNSHIP_PHRASES),
    (Audience.CAREER_CHANGE, _CAREER_CHANGE_WORDS, _CAREER_CHANGE_PHRASES),
    (Audience.LINKEDIN, _LINKEDIN_WORDS, _LINKEDIN_PHRASES),
    (Audience.WHATSAPP, _WHATSAPP_WORDS, _WHATSAPP_PHRASES),
    (Audience.NETWORKING_CAREER, _NETWORKING_CAREER_WORDS, _NETWORKING_CAREER_PHRASES),
    (Audience.HOA_LIVING, _HOA_LIVING_WORDS, _HOA_LIVING_PHRASES),
    (Audience.COOP_HOUSING, _COOP_HOUSING_WORDS, _COOP_HOUSING_PHRASES),
    (Audience.COMMUNITY_GARDEN, _COMMUNITY_GARDEN_WORDS, _COMMUNITY_GARDEN_PHRASES),
    (Audience.MUTUAL_AID, _MUTUAL_AID_WORDS, _MUTUAL_AID_PHRASES),
    (Audience.FISHKEEPING, _FISHKEEPING_WORDS, _FISHKEEPING_PHRASES),
    (Audience.TERRARIUM, _TERRARIUM_WORDS, _TERRARIUM_PHRASES),
    (Audience.ANTKEEPING, _ANTKEEPING_WORDS, _ANTKEEPING_PHRASES),
    (Audience.BEEKEEPING_ADVANCED, _BEEKEEPING_ADVANCED_WORDS, _BEEKEEPING_ADVANCED_PHRASES),
    (Audience.FOUNTAIN_PEN, _FOUNTAIN_PEN_WORDS, _FOUNTAIN_PEN_PHRASES),
    (Audience.STATIONERY, _STATIONERY_WORDS, _STATIONERY_PHRASES),
    (Audience.MECHANICAL_KEYBOARD, _MECHANICAL_KEYBOARD_WORDS, _MECHANICAL_KEYBOARD_PHRASES),
    (Audience.PC_BUILDING, _PC_BUILDING_WORDS, _PC_BUILDING_PHRASES),
    (Audience.HOME_LAB, _HOME_LAB_WORDS, _HOME_LAB_PHRASES),
    (Audience.THREE_D_MODELING, _THREE_D_MODELING_WORDS, _THREE_D_MODELING_PHRASES),
    (Audience.CNC, _CNC_WORDS, _CNC_PHRASES),
    (Audience.LASER_CUTTING, _LASER_CUTTING_WORDS, _LASER_CUTTING_PHRASES),
    (Audience.RESIN_PRINTING, _RESIN_PRINTING_WORDS, _RESIN_PRINTING_PHRASES),
    (Audience.FILAMENT_PRINTING, _FILAMENT_PRINTING_WORDS, _FILAMENT_PRINTING_PHRASES),
    (Audience.MEDITATION, _MEDITATION_WORDS, _MEDITATION_PHRASES),
    (Audience.STOICISM, _STOICISM_WORDS, _STOICISM_PHRASES),
    (Audience.JOURNAL_PROMPTS, _JOURNAL_PROMPTS_WORDS, _JOURNAL_PROMPTS_PHRASES),
    (Audience.HABIT_BUILDING, _HABIT_BUILDING_WORDS, _HABIT_BUILDING_PHRASES),
    (Audience.TIME_BLOCKING, _TIME_BLOCKING_WORDS, _TIME_BLOCKING_PHRASES),
    (Audience.SECOND_BRAIN, _SECOND_BRAIN_WORDS, _SECOND_BRAIN_PHRASES),
    (Audience.PACKING, _PACKING_WORDS, _PACKING_PHRASES),
    (Audience.TRAVEL_PHOTOGRAPHY, _TRAVEL_PHOTOGRAPHY_WORDS, _TRAVEL_PHOTOGRAPHY_PHRASES),
    (Audience.SOLO_TRAVEL, _SOLO_TRAVEL_WORDS, _SOLO_TRAVEL_PHRASES),
    (Audience.FAMILY_TRAVEL, _FAMILY_TRAVEL_WORDS, _FAMILY_TRAVEL_PHRASES),
    (Audience.BUDGET_TRAVEL, _BUDGET_TRAVEL_WORDS, _BUDGET_TRAVEL_PHRASES),
    (Audience.POINTS_MILES, _POINTS_MILES_WORDS, _POINTS_MILES_PHRASES),
    (Audience.REAL_ESTATE_PHOTO, _REAL_ESTATE_PHOTO_WORDS, _REAL_ESTATE_PHOTO_PHRASES),
    (Audience.STAGING, _STAGING_WORDS, _STAGING_PHRASES),
    (Audience.INTERIOR_STYLING, _INTERIOR_STYLING_WORDS, _INTERIOR_STYLING_PHRASES),
    (Audience.EVENT_PHOTOGRAPHY, _EVENT_PHOTOGRAPHY_WORDS, _EVENT_PHOTOGRAPHY_PHRASES),
    (Audience.PORTRAIT_PHOTO, _PORTRAIT_PHOTO_WORDS, _PORTRAIT_PHOTO_PHRASES),
    (Audience.STREET_PHOTO, _STREET_PHOTO_WORDS, _STREET_PHOTO_PHRASES),
    (Audience.WILDLIFE_PHOTO, _WILDLIFE_PHOTO_WORDS, _WILDLIFE_PHOTO_PHRASES),
    (Audience.ASTRO_IMAGING_PROC, _ASTRO_IMAGING_PROC_WORDS, _ASTRO_IMAGING_PROC_PHRASES),
]


def score_audiences(text: str) -> dict[Audience, int]:
    """Raw scores per persona (for tests / debugging)."""
    raw = (text or "").strip()
    low = raw.lower()
    toks = _tokens(raw)
    scores: dict[Audience, int] = {a: 0 for a in Audience}
    scores[Audience.CODE] = _score_code(raw, low, toks)
    for aud, words, phrases in _LEXICONS:
        scores[aud] = _score_lexicon(low, toks, words, phrases)

    # "work on tsearch-revival" / "work on my-app" — project folders are code
    # work by default, not backpacking/plain specialty noise.
    m_work = re.search(
        r"\b(?:work|working)\s+on\s+([a-z0-9][\w./-]{1,64})",
        low,
    )
    if m_work:
        subj = m_work.group(1).strip("/").strip()
        taskish = any(
            w in subj
            for w in (
                "message", "email", "inbox", "reply", "linkedin", "whatsapp",
                "fitness", "workout", "recipe", "meal", "travel", "trip",
            )
        )
        if subj and not taskish and (
            "-" in subj or "/" in subj or "_" in subj or subj.isidentifier()
        ):
            scores[Audience.CODE] = max(scores[Audience.CODE], 4)

    # Disambiguation nudges
    # Cover letter / resume → job, not generic writer
    # Bare "linkedin" is NOT a job signal (messaging is LinkedIn mode).
    if any(p in low for p in ("cover letter", "resume", "cv ", "interview")):
        scores[Audience.JOB] += 3
        scores[Audience.WRITER] = max(0, scores[Audience.WRITER] - 2)
    if any(p in low for p in ("linkedin summary", "linkedin profile", "open to work", "resume on linkedin")):
        scores[Audience.JOB] += 3
        scores[Audience.LINKEDIN] = max(0, scores[Audience.LINKEDIN] - 1)
    # SQL + code files → prefer CODE over DATA when both present
    if scores[Audience.CODE] >= 4 and scores[Audience.DATA] > 0:
        if _CODE_TOKEN_RE.search(raw) or _CODE_INTENT_RE.search(raw):
            scores[Audience.CODE] += 2
    # "lesson plan" → teacher over student
    if "lesson plan" in low or "for my students" in low:
        scores[Audience.TEACHER] += 4
    # Marketing campaigns vs general business
    if any(p in low for p in ("ad copy", "seo ", "paid social", "content calendar", "utm")):
        scores[Audience.MARKETING] += 3
        scores[Audience.BUSINESS] = max(0, scores[Audience.BUSINESS] - 1)
    # Sales outbound
    if any(p in low for p in ("cold email", "discovery call", "sales pipeline", "objection")):
        scores[Audience.SALES] += 4
    # Product management language
    if any(p in low for p in ("user story", "prd", "product roadmap", "acceptance criteria")):
        scores[Audience.PRODUCT] += 4
        scores[Audience.FOUNDER] = max(0, scores[Audience.FOUNDER] - 1)
    # Language learning vs writer grammar
    if any(p in low for p in ("translate this", "how do you say", "learn spanish", "learn french")):
        scores[Audience.LANGUAGE] += 4
        scores[Audience.WRITER] = max(0, scores[Audience.WRITER] - 2)
    # Science lab vs student homework
    if any(p in low for p in ("lab protocol", "experimental design", "balance this equation")):
        scores[Audience.SCIENCE] += 3
    # Music craft over generic creative
    if any(p in low for p in ("chord progression", "music production", "song structure", "mix this track")):
        scores[Audience.MUSIC] += 4
        scores[Audience.CREATIVE] = max(0, scores[Audience.CREATIVE] - 2)
    # Cybersecurity: oauth/mfa in security phrases stay security; pure code files stay code
    if any(p in low for p in ("threat model", "penetration test", "security audit", "data breach")):
        scores[Audience.SECURITY] += 4
    # Physical engineering vs software
    if any(p in low for p in ("mechanical design", "load calculation", "solidworks", "civil")):
        scores[Audience.ENGINEERING] += 3
        if scores[Audience.CODE] < 6:
            scores[Audience.CODE] = max(0, scores[Audience.CODE] - 2)
    # Accessibility vs design
    if any(p in low for p in ("wcag", "screen reader", "alt text", "a11y")):
        scores[Audience.ACCESSIBILITY] += 4
        scores[Audience.DESIGN] = max(0, scores[Audience.DESIGN] - 1)
    # Journalism vs writer
    if any(p in low for p in ("press release", "news story", "fact check", "on the record")):
        scores[Audience.JOURNALISM] += 4
        scores[Audience.WRITER] = max(0, scores[Audience.WRITER] - 2)
    # HR vs business
    if any(p in low for p in ("employee handbook", "performance review", "hr policy", "people ops")):
        scores[Audience.HR] += 4
    # DIY vs engineering
    if any(p in low for p in ("how to fix", "home repair", "ikea", "diy project")):
        scores[Audience.DIY] += 3
    # Tax vs general finance
    if any(p in low for p in ("tax return", "file taxes", "1040", "w-2", "1099", "irs")):
        scores[Audience.TAX] += 5
        scores[Audience.FINANCE] = max(0, scores[Audience.FINANCE] - 2)
    # Crypto vs investing
    if any(p in low for p in ("bitcoin", "ethereum", "defi", "nft", "web3", "smart contract")):
        scores[Audience.CRYPTO] += 4
        scores[Audience.INVESTING] = max(0, scores[Audience.INVESTING] - 2)
    # Photography vs creative
    if any(p in low for p in ("camera settings", "aperture", "lightroom", "portrait lighting")):
        scores[Audience.PHOTOGRAPHY] += 4
        scores[Audience.CREATIVE] = max(0, scores[Audience.CREATIVE] - 2)
    # Film vs creative
    if any(p in low for p in ("short film", "screenplay", "color grade", "shot list")):
        scores[Audience.FILM] += 4
        scores[Audience.CREATIVE] = max(0, scores[Audience.CREATIVE] - 2)
    # Baking vs cooking
    if any(p in low for p in ("sourdough", "bake bread", "cake recipe", "pastry")):
        scores[Audience.BAKING] += 4
        scores[Audience.COOKING] = max(0, scores[Audience.COOKING] - 2)
    # Therapy vs health
    if any(p in low for p in ("therapy", "anxiety", "depression", "coping", "mental health")):
        scores[Audience.THERAPY] += 3
    # Immigration vs legal
    if any(p in low for p in ("visa", "green card", "h-1b", "h1b", "uscis", "f-1")):
        scores[Audience.IMMIGRATION] += 4
        scores[Audience.LEGAL] = max(0, scores[Audience.LEGAL] - 1)
    # Legal docs even when "plain English" is requested
    if any(p in low for p in ("nda", "non-disclosure", "indemnity", "indemnification", "terms of service")):
        scores[Audience.LEGAL] += 3
    # Math vs science/code
    if any(p in low for p in ("prove that", "calculus", "algebra", "geometry proof")):
        scores[Audience.MATH] += 4
    # Ecommerce vs retail
    if any(p in low for p in ("shopify", "amazon listing", "abandoned cart", "woocommerce")):
        scores[Audience.ECOMMERCE] += 4
        scores[Audience.RETAIL] = max(0, scores[Audience.RETAIL] - 2)
    # UX research vs design
    if any(p in low for p in ("usability test", "user interview", "user research", "affinity map")):
        scores[Audience.UX_RESEARCH] += 4
        scores[Audience.DESIGN] = max(0, scores[Audience.DESIGN] - 1)
    # Investing vs finance
    if any(p in low for p in ("index fund", "etf", "stock portfolio", "asset allocation", "brokerage")):
        scores[Audience.INVESTING] += 5
        scores[Audience.FINANCE] = max(0, scores[Audience.FINANCE] - 2)
    # DevOps / cloud / infra vs general Ultracode
    if any(p in low for p in ("terraform", "ci cd", "ci/cd", "kubernetes", "helm", "ansible", "pipeline deploy")):
        scores[Audience.DEVOPS] += 10
        if scores[Audience.CODE] < 14:
            scores[Audience.CODE] = max(0, scores[Audience.CODE] - 5)
    if any(p in low for p in ("aws ", "gcp ", "azure ", "cloud architecture", "s3 bucket")):
        scores[Audience.CLOUD] += 4
    if any(p in low for p in ("postgres", "mysql", "slow query", "database schema")):
        scores[Audience.DATABASE] += 4
    if any(p in low for p in ("subnet", "dns issue", "wifi", "vlan", "dhcp")):
        scores[Audience.NETWORKING] += 4
    if any(p in low for p in ("ios app", "android app", "react native", "flutter")):
        scores[Audience.MOBILE] += 4
    if any(p in low for p in ("react ", "nextjs", "frontend", "backend api", "web app")):
        scores[Audience.WEBDEV] += 3
    if any(p in low for p in ("esp32", "stm32", "firmware", "bare metal")):
        scores[Audience.EMBEDDED] += 4
    if any(p in low for p in ("obsidian", "zettelkasten", "second brain", "logseq")):
        scores[Audience.PKM] += 5
    if any(p in low for p in ("d&d", "dungeons and dragons", "pathfinder", "character sheet")):
        scores[Audience.TABLETOP] += 4
    if any(p in low for p in ("chess opening", "checkmate", "endgame")):
        scores[Audience.CHESS] += 4

    # Wave 6 disambiguation
    if any(p in low for p in ("workout plan", "gym program", "hypertrophy", "leg day")):
        scores[Audience.FITNESS] += 4
        scores[Audience.SPORTS] = max(0, scores[Audience.SPORTS] - 1)
    if any(p in low for p in ("yoga sequence", "vinyasa", "yoga pose")):
        scores[Audience.YOGA] += 4
        scores[Audience.FITNESS] = max(0, scores[Audience.FITNESS] - 1)
    if any(p in low for p in ("bouldering", "climbing beta", "belay")):
        scores[Audience.CLIMBING] += 4
    if any(p in low for p in ("unity", "godot", "game design doc", "game loop")):
        scores[Audience.GAME_DEV] += 4
        scores[Audience.GAMING] = max(0, scores[Audience.GAMING] - 2)
    if any(p in low for p in ("train a model", "fine tune", "pytorch", "rag pipeline", "llm")):
        scores[Audience.ML_AI] += 5
        if scores[Audience.CODE] < 8:
            scores[Audience.CODE] = max(0, scores[Audience.CODE] - 1)
    if any(p in low for p in ("error budget", "slo ", "postmortem", "on-call")):
        scores[Audience.SRE] += 5
        scores[Audience.DEVOPS] = max(0, scores[Audience.DEVOPS] - 1)
    if any(p in low for p in ("system design interview", "design twitter", "cap theorem")):
        scores[Audience.SYSTEM_DESIGN] += 5
    if any(p in low for p in ("youtube script", "tiktok idea", "grow my channel")):
        scores[Audience.CONTENT_CREATOR] += 4
        scores[Audience.MARKETING] = max(0, scores[Audience.MARKETING] - 1)
    if any(p in low for p in ("seo audit", "keyword research", "on-page seo")):
        scores[Audience.SEO] += 4
        scores[Audience.MARKETING] = max(0, scores[Audience.MARKETING] - 1)
    if any(p in low for p in ("salary negotiation", "negotiate offer", "batna")):
        scores[Audience.NEGOTIATION] += 4
        scores[Audience.JOB] = max(0, scores[Audience.JOB] - 1)
    if any(p in low for p in ("adhd tips", "neurodivergent", "executive function")):
        scores[Audience.NEURODIVERSITY] += 4
    if any(p in low for p in ("smoke a brisket", "bbq ribs", "smoker temperature")):
        scores[Audience.BBQ] += 4
        scores[Audience.COOKING] = max(0, scores[Audience.COOKING] - 2)
    if any(p in low for p in ("home assistant", "smart home setup", "automate lights")):
        scores[Audience.SMART_HOME] += 4
        scores[Audience.IOT] = max(0, scores[Audience.IOT] - 1)
    if any(p in low for p in ("powerlifting", "1rm", "meet prep")):
        scores[Audience.POWERLIFTING] += 4
        scores[Audience.FITNESS] = max(0, scores[Audience.FITNESS] - 1)
    if any(p in low for p in ("clogged drain", "replace faucet", "water heater")):
        scores[Audience.PLUMBING] += 4
        scores[Audience.DIY] = max(0, scores[Audience.DIY] - 1)
    if any(p in low for p in ("breaker tripped", "wire a switch", "electrical outlet")):
        scores[Audience.ELECTRICAL_TRADE] += 4
        scores[Audience.DIY] = max(0, scores[Audience.DIY] - 1)
    if any(p in low for p in ("ac not cooling", "heat pump", "hvac")):
        scores[Audience.HVAC] += 4
    if any(p in low for p in ("write a poem", "haiku about", "revise this poem")):
        scores[Audience.POETRY] += 4
        scores[Audience.WRITER] = max(0, scores[Audience.WRITER] - 2)
    if any(p in low for p in ("term sheet", "seed round", "cap table")):
        scores[Audience.VC] += 4
        scores[Audience.FOUNDER] = max(0, scores[Audience.FOUNDER] - 1)
    if any(p in low for p in ("orbital mechanics", "rocket equation", "launch vehicle")):
        scores[Audience.SPACEFLIGHT] += 4
        scores[Audience.ASTRONOMY] = max(0, scores[Audience.ASTRONOMY] - 1)
    if any(p in low for p in ("prometheus", "opentelemetry", "distributed tracing")):
        scores[Audience.OBSERVABILITY] += 4
        scores[Audience.DEVOPS] = max(0, scores[Audience.DEVOPS] - 1)
    if any(p in low for p in ("sprint planning", "kanban board", "story points")):
        scores[Audience.AGILE] += 4
        scores[Audience.PROJECT_MGMT] = max(0, scores[Audience.PROJECT_MGMT] - 1)
    if any(p in low for p in ("mixology", "negroni", "old fashioned", "classic cocktail")):
        scores[Audience.COCKTAILS] += 4
        scores[Audience.BARTENDING] = max(0, scores[Audience.BARTENDING] - 2)


    # Wave 7 disambiguation
    if any(p in low for p in ("tennis lesson", "forehand technique", "improve serve")):
        scores[Audience.TENNIS] += 4
        scores[Audience.SPORTS] = max(0, scores[Audience.SPORTS] - 1)
    if any(p in low for p in ("soccer drills", "football tactics", "set piece")):
        scores[Audience.SOCCER] += 4
    if any(p in low for p in ("crossfit wod", "amrap", "scale this wod")):
        scores[Audience.CROSSFIT] += 4
        scores[Audience.FITNESS] = max(0, scores[Audience.FITNESS] - 1)
    if any(p in low for p in ("write a screenplay", "beat sheet", "logline help")):
        scores[Audience.SCREENWRITING] += 4
        scores[Audience.FILM] = max(0, scores[Audience.FILM] - 1)
        scores[Audience.WRITER] = max(0, scores[Audience.WRITER] - 1)
    if any(p in low for p in ("write a novel", "novel outline", "chapter revision")):
        scores[Audience.NOVEL] += 4
        scores[Audience.WRITER] = max(0, scores[Audience.WRITER] - 1)
    if any(p in low for p in ("make a budget", "pay off debt", "emergency fund")):
        scores[Audience.PERSONAL_FINANCE] += 5
        scores[Audience.FINANCE] = max(0, scores[Audience.FINANCE] - 2)
        scores[Audience.INVESTING] = max(0, scores[Audience.INVESTING] - 1)
    if any(p in low for p in ("401k", "roth ira", "retirement plan")):
        scores[Audience.RETIREMENT] += 4
        scores[Audience.INVESTING] = max(0, scores[Audience.INVESTING] - 1)
    if any(p in low for p in ("data pipeline", "dbt model", "etl design", "airflow")):
        scores[Audience.DATA_ENGINEERING] += 5
        scores[Audience.DATA] = max(0, scores[Audience.DATA] - 2)
    if any(p in low for p in ("excel formula", "pivot table", "google sheets")):
        scores[Audience.SPREADSHEETS] += 4
        scores[Audience.DATA] = max(0, scores[Audience.DATA] - 1)
    if any(p in low for p in ("kubernetes deployment", "kubectl", "helm chart", "k8s ")):
        scores[Audience.KUBERNETES] += 5
        scores[Audience.DEVOPS] = max(0, scores[Audience.DEVOPS] - 2)
        scores[Audience.CLOUD] = max(0, scores[Audience.CLOUD] - 1)
    if any(p in low for p in ("prompt engineering", "system prompt", "few shot")):
        scores[Audience.PROMPT_ENG] += 4
        scores[Audience.ML_AI] = max(0, scores[Audience.ML_AI] - 1)
    if any(p in low for p in ("train my dog", "puppy training", "leash training")):
        scores[Audience.DOG_TRAINING] += 4
        scores[Audience.PETS] = max(0, scores[Audience.PETS] - 1)
    if any(p in low for p in ("ableton", "mix this track", "music production", "mastering basics", "daw", "vst")):
        scores[Audience.MUSIC_PRODUCTION] += 6
        scores[Audience.MUSIC] = max(0, scores[Audience.MUSIC] - 4)
    if any(p in low for p in ("meal prep", "batch cooking", "weekly meal prep")):
        scores[Audience.MEAL_PREP] += 4
        scores[Audience.COOKING] = max(0, scores[Audience.COOKING] - 1)
    if any(p in low for p in ("password manager", "enable 2fa", "passkey setup", "passkey", "password vault")):
        scores[Audience.PASSWORD_SECURITY] += 6
        scores[Audience.CYBER_HYGIENE] = max(0, scores[Audience.CYBER_HYGIENE] - 3)
        scores[Audience.SECURITY] = max(0, scores[Audience.SECURITY] - 1)
    if any(p in low for p in ("frame a wall", "install baseboard", "trim work")):
        scores[Audience.CARPENTRY] += 4
        scores[Audience.DIY] = max(0, scores[Audience.DIY] - 1)
        scores[Audience.WOODWORKING] = max(0, scores[Audience.WOODWORKING] - 1)
    if any(p in low for p in ("design an api", "openapi spec", "rest best practices")):
        scores[Audience.API_DESIGN] += 4
        scores[Audience.WEBDEV] = max(0, scores[Audience.WEBDEV] - 1)
        scores[Audience.BACKEND] = max(0, scores[Audience.BACKEND] - 1)
    if any(p in low for p in ("hiking gear", "day hike", "trail plan")):
        scores[Audience.HIKING] += 4
        scores[Audience.OUTDOORS] = max(0, scores[Audience.OUTDOORS] - 1)
    if any(p in low for p in ("snatch technique", "clean and jerk", "olympic lifting")):
        scores[Audience.OLYMPIC_LIFTING] += 4
        scores[Audience.POWERLIFTING] = max(0, scores[Audience.POWERLIFTING] - 1)
        scores[Audience.FITNESS] = max(0, scores[Audience.FITNESS] - 1)


    # Wave 8 disambiguation
    if any(p in low for p in ("pickleball", "third shot drop", "kitchen rules")):
        scores[Audience.PICKLEBALL] += 4
        scores[Audience.SPORTS] = max(0, scores[Audience.SPORTS] - 1)
    if any(p in low for p in ("learn spanish", "spanish conjugation", "in spanish")):
        scores[Audience.SPANISH] += 5
        scores[Audience.LANGUAGE] = max(0, scores[Audience.LANGUAGE] - 2)
    if any(p in low for p in ("learn french", "french conjugation", "in french", "français", "francais")):
        scores[Audience.FRENCH] += 5
        scores[Audience.LANGUAGE] = max(0, scores[Audience.LANGUAGE] - 2)
    if any(p in low for p in ("learn german", "german cases", "in german", "deutsch")):
        scores[Audience.GERMAN] += 5
        scores[Audience.LANGUAGE] = max(0, scores[Audience.LANGUAGE] - 2)
    if any(p in low for p in ("learn japanese", "hiragana", "kanji study")):
        scores[Audience.JAPANESE] += 5
        scores[Audience.LANGUAGE] = max(0, scores[Audience.LANGUAGE] - 2)
    if any(p in low for p in ("learn mandarin", "pinyin tones", "hsk study")):
        scores[Audience.MANDARIN] += 5
        scores[Audience.LANGUAGE] = max(0, scores[Audience.LANGUAGE] - 2)
    if any(p in low for p in ("learn korean", "hangul", "topik study")):
        scores[Audience.KOREAN] += 5
        scores[Audience.LANGUAGE] = max(0, scores[Audience.LANGUAGE] - 2)
    if any(p in low for p in ("learn italian", "italian conjugation", "in italian")):
        scores[Audience.ITALIAN] += 5
        scores[Audience.LANGUAGE] = max(0, scores[Audience.LANGUAGE] - 2)
    if any(p in low for p in ("learn portuguese", "brazilian portuguese", "portuguese conjugation")):
        scores[Audience.PORTUGUESE] += 5
        scores[Audience.LANGUAGE] = max(0, scores[Audience.LANGUAGE] - 2)
    if any(p in low for p in ("fresh pasta", "risotto technique", "ragu recipe")):
        scores[Audience.ITALIAN_COOKING] += 4
        scores[Audience.COOKING] = max(0, scores[Audience.COOKING] - 2)
    if any(p in low for p in ("make dashi", "sushi rice", "miso soup")):
        scores[Audience.JAPANESE_COOKING] += 4
        scores[Audience.COOKING] = max(0, scores[Audience.COOKING] - 2)
    if any(p in low for p in ("bake bread", "sourdough loaf", "bread formula")):
        scores[Audience.BREAD] += 4
        scores[Audience.BAKING] = max(0, scores[Audience.BAKING] - 2)
    if any(p in low for p in ("croissant", "laminated dough", "choux pastry")):
        scores[Audience.PASTRY] += 4
        scores[Audience.BAKING] = max(0, scores[Audience.BAKING] - 2)
    if any(p in low for p in ("rust ownership", "borrow checker", "rust lifetimes")):
        scores[Audience.RUST_LANG] += 5
        scores[Audience.CODE] = max(0, scores[Audience.CODE] - 2)
    if any(p in low for p in ("goroutines", "go modules", "golang service")):
        scores[Audience.GO_LANG] += 5
        scores[Audience.CODE] = max(0, scores[Audience.CODE] - 2)
    if any(p in low for p in ("pandas dataframe", "groupby pandas", "jupyter notebook")):
        scores[Audience.PYTHON_DATA] += 5
        scores[Audience.DATA] = max(0, scores[Audience.DATA] - 2)
        scores[Audience.CODE] = max(0, scores[Audience.CODE] - 1)
    if any(p in low for p in ("terraform module", "terraform state", "terraform plan", "terraform ", " hcl ")):
        scores[Audience.TERRAFORM] += 8
        scores[Audience.DEVOPS] = max(0, scores[Audience.DEVOPS] - 4)
    if any(p in low for p in ("dockerfile", "docker compose", "build image")):
        scores[Audience.DOCKER] += 5
        scores[Audience.DEVOPS] = max(0, scores[Audience.DEVOPS] - 1)
        scores[Audience.CODE] = max(0, scores[Audience.CODE] - 1)
    if any(p in low for p in ("kubectl", "helm chart", "kubernetes deployment", "k8s ", "kubernetes ")):
        scores[Audience.KUBERNETES] += 8
        scores[Audience.DEVOPS] = max(0, scores[Audience.DEVOPS] - 4)
        scores[Audience.CLOUD] = max(0, scores[Audience.CLOUD] - 1)
    # Broad multi-tool infra + CI/CD → DevOps umbrella over single tools
    if (
        any(p in low for p in ("ci/cd", "ci cd", "pipeline"))
        and sum(1 for p in ("terraform", "kubernetes", "helm", "ansible", "docker") if p in low) >= 2
    ):
        scores[Audience.DEVOPS] += 10
        scores[Audience.KUBERNETES] = max(0, scores[Audience.KUBERNETES] - 3)
        scores[Audience.TERRAFORM] = max(0, scores[Audience.TERRAFORM] - 3)
        scores[Audience.DOCKER] = max(0, scores[Audience.DOCKER] - 2)
    if any(p in low for p in ("pc build", "choose a gpu", "cable management")):
        scores[Audience.PC_BUILDING] += 4
        scores[Audience.ELECTRONICS] = max(0, scores[Audience.ELECTRONICS] - 1)
    if any(p in low for p in ("homelab", "proxmox", "self hosted")):
        scores[Audience.HOME_LAB] += 4
    if any(p in low for p in ("linkedin profile", "linkedin headline", "connection request")):
        scores[Audience.LINKEDIN] += 4
        scores[Audience.JOB] = max(0, scores[Audience.JOB] - 1)
    # Messaging / inbox on LinkedIn is product use, not job-search coaching
    if any(
        p in low
        for p in (
            "linkedin message", "linkedin messages", "linkedin inbox",
            "linkedin dm", "inmail", "respond to my linkedin",
            "reply to linkedin", "reply to my linkedin",
            "answer linkedin", "check linkedin messages",
        )
    ) or ("linkedin" in low and any(
        p in low for p in ("message", "messages", "inbox", "reply", "respond", "dm")
    )):
        scores[Audience.LINKEDIN] += 8
        scores[Audience.JOB] = max(0, scores[Audience.JOB] - 5)
        scores[Audience.MARKETING] = max(0, scores[Audience.MARKETING] - 2)
        scores[Audience.EMAIL_PRODUCTIVITY] = max(0, scores[Audience.EMAIL_PRODUCTIVITY] - 2)
        scores[Audience.WHATSAPP] = max(0, scores[Audience.WHATSAPP] - 3)
    # WhatsApp chat replies — not LinkedIn, not support desk
    if (
        "whatsapp" in low
        or "whats app" in low
        or any(
            p in low
            for p in (
                "reply to my whatsapp", "respond to my whatsapp",
                "whatsapp messages", "whatsapp message", "whatsapp chat",
                "check whatsapp", "open whatsapp",
            )
        )
    ):
        scores[Audience.WHATSAPP] += 10
        scores[Audience.LINKEDIN] = max(0, scores[Audience.LINKEDIN] - 8)
        scores[Audience.MARKETING] = max(0, scores[Audience.MARKETING] - 3)
        scores[Audience.SUPPORT] = max(0, scores[Audience.SUPPORT] - 2)
        scores[Audience.EMAIL_PRODUCTIVITY] = max(0, scores[Audience.EMAIL_PRODUCTIVITY] - 2)
    if any(p in low for p in ("career change", "career pivot", "transferable skills")):
        scores[Audience.CAREER_CHANGE] += 4
        scores[Audience.JOB] = max(0, scores[Audience.JOB] - 1)
    if any(p in low for p in ("houseplant care", "repot plant", "succulent care")):
        scores[Audience.HOUSEPLANTS] += 4
        scores[Audience.GARDENING] = max(0, scores[Audience.GARDENING] - 1)
    if any(p in low for p in ("build a habit", "habit stack", "habit tracker")):
        scores[Audience.HABIT_BUILDING] += 4
        scores[Audience.PRODUCTIVITY] = max(0, scores[Audience.PRODUCTIVITY] - 1)
    if any(p in low for p in ("time blocking", "deep work schedule", "theme days")):
        scores[Audience.TIME_BLOCKING] += 4
        scores[Audience.PRODUCTIVITY] = max(0, scores[Audience.PRODUCTIVITY] - 1)
    if any(p in low for p in ("improve lcp", "core web vitals", "lighthouse score")):
        scores[Audience.PERFORMANCE_WEB] += 4
        scores[Audience.WEBDEV] = max(0, scores[Audience.WEBDEV] - 1)
        scores[Audience.FRONTEND] = max(0, scores[Audience.FRONTEND] - 1)
    if any(p in low for p in ("graphic design", "poster layout", "typography tips")):
        scores[Audience.GRAPHIC_DESIGN] += 4
        scores[Audience.DESIGN] = max(0, scores[Audience.DESIGN] - 1)
    if any(p in low for p in ("ux writing", "microcopy", "error message copy")):
        scores[Audience.UX_WRITING] += 4
        scores[Audience.WRITER] = max(0, scores[Audience.WRITER] - 1)
        scores[Audience.DESIGN] = max(0, scores[Audience.DESIGN] - 1)
    if any(p in low for p in ("esports practice", "vod review", "scrim schedule")):
        scores[Audience.ESPORTS] += 4
        scores[Audience.GAMING] = max(0, scores[Audience.GAMING] - 2)
    if any(p in low for p in ("speedrun route", "any percent", "split analysis")):
        scores[Audience.SPEEDRUNNING] += 4
        scores[Audience.GAMING] = max(0, scores[Audience.GAMING] - 2)
    if any(p in low for p in ("buy a car", "out the door price", "negotiate car")):
        scores[Audience.CAR_BUYING] += 4
        scores[Audience.AUTOMOTIVE] = max(0, scores[Audience.AUTOMOTIVE] - 1)
    if any(p in low for p in ("make an offer", "home inspection", "closing checklist")):
        scores[Audience.HOME_BUYING] += 4
        scores[Audience.REAL_ESTATE] = max(0, scores[Audience.REAL_ESTATE] - 1)
    if any(p in low for p in ("college essay", "common app", "school list")):
        scores[Audience.COLLEGE_APPS] += 4
        scores[Audience.STUDENT] = max(0, scores[Audience.STUDENT] - 1)
        scores[Audience.HIGHER_ED] = max(0, scores[Audience.HIGHER_ED] - 1)

    # Self-label boosts
    m = _SELF_LABEL.search(low)
    if m:
        role = m.group("role").lower()
        boosts = [
            (("developer", "programmer", "sre", "devops"), Audience.CODE),
            (("security engineer", "pentester"), Audience.SECURITY),
            (("mechanical engineer", "civil engineer"), Audience.ENGINEERING),
            (("professor", "academic", "researcher", "scholar"), Audience.ACADEMIC),
            (("student", "undergrad", "grad"), Audience.STUDENT),
            (("journalist",), Audience.JOURNALISM),
            (("writer", "novelist", "blogger", "copywriter"), Audience.WRITER),
            (("designer", "ux"), Audience.DESIGN),
            (("founder", "co-founder", "cofounder", "ceo"), Audience.FOUNDER),
            (("product manager", "pm"), Audience.PRODUCT),
            (("marketer", "marketing"), Audience.MARKETING),
            (("salesperson", "sales", "ae", "sdr"), Audience.SALES),
            (("manager", "executive"), Audience.BUSINESS),
            (("parent", "mom", "dad", "mother", "father"), Audience.PARENT),
            (("lawyer", "attorney"), Audience.LEGAL),
            (("teacher", "educator"), Audience.TEACHER),
            (("nurse", "doctor", "physician", "clinician"), Audience.HEALTH),
            (("scientist", "physicist", "chemist", "biologist"), Audience.SCIENCE),
            (("support agent", "customer success"), Audience.SUPPORT),
            (("data", "analyst"), Audience.DATA),
            (("musician",), Audience.MUSIC),
            (("filmmaker", "photographer", "artist"), Audience.CREATIVE),
            (("realtor", "real estate"), Audience.REAL_ESTATE),
            (("chef", "cook"), Audience.COOKING),
            (("gamer", "game designer"), Audience.GAMING),
            (("coach", "athlete"), Audience.SPORTS),
            (("hr", "people ops"), Audience.HR),
            (("farmer",), Audience.AGRICULTURE),
            (("mechanic",), Audience.AUTOMOTIVE),
            (("engineer",), Audience.CODE),  # generic engineer → code last
        ]
        for keys, aud in boosts:
            if any(k in role for k in keys):
                scores[aud] += 8
                break
    return scores


def detect_audience(text: str) -> Audience:
    """Pick the best-matching persona for this ask (default PLAIN)."""
    raw = (text or "").strip()
    if not raw:
        return Audience.PLAIN
    low = raw.lower()

    scores = score_audiences(raw)
    ranked = sorted(
        (
            (aud, sc)
            for aud, sc in scores.items()
            if aud is not Audience.PLAIN and sc >= 2
        ),
        key=lambda x: (-x[1], _PRIORITY.index(x[0]) if x[0] in _PRIORITY else 99),
    )

    # “I’m not a developer / keep it simple” forces plain *unless* another
    # persona is clearly present (e.g. “NDA in plain English” → legal).
    plain_force = any(p in low for p in _PLAIN_PHRASES)
    if plain_force:
        best_special = ranked[0][1] if ranked else 0
        if best_special < 4:
            return Audience.PLAIN

    if not ranked:
        return Audience.PLAIN
    best_aud, best_sc = ranked[0]
    # Academic vs student: research language wins academic
    if best_aud is Audience.STUDENT and scores[Audience.ACADEMIC] >= best_sc - 1:
        if scores[Audience.ACADEMIC] >= 4:
            return Audience.ACADEMIC
    # Founder vs business: fundraising/startup language wins founder
    if best_aud is Audience.BUSINESS and scores[Audience.FOUNDER] >= best_sc - 1:
        if scores[Audience.FOUNDER] >= 4:
            return Audience.FOUNDER
    # Product vs founder: PRD/roadmap without fundraising stays product
    if best_aud is Audience.FOUNDER and scores[Audience.PRODUCT] >= best_sc:
        if scores[Audience.PRODUCT] >= 4 and scores[Audience.FOUNDER] < scores[Audience.PRODUCT] + 2:
            return Audience.PRODUCT
    # Marketing vs writer for social/SEO
    if best_aud is Audience.WRITER and scores[Audience.MARKETING] >= best_sc - 1:
        if scores[Audience.MARKETING] >= 4:
            return Audience.MARKETING
    # Code preferred on exact ties
    for aud, sc in ranked:
        if sc == best_sc and aud is Audience.CODE:
            return Audience.CODE
    return best_aud


_SHAPE: dict[Audience, str] = {
    Audience.CODE: (
        "[Ultracode mode] The person asking is comfortable with code and "
        "wants a real change in this project folder. Read the relevant files, "
        "make (or precisely describe) the edit, and name paths and symbols. "
        "Prefer doing the work over high-level advice. Keep explanations "
        "tight and technical.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ACADEMIC: (
        "[Academic mode] Research or scholarly context. Precise, careful, "
        "well-structured. Clear argumentation; do not invent citations.\n\n"
        "Their request:\n{text}"
    ),
    Audience.STUDENT: (
        "[Student mode] Learning / coursework. Teach with short steps and "
        "explanations; support understanding, not just a finished dump.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WRITER: (
        "[Writer mode] Prose, tone, and audience. Structure, clarity, voice; "
        "offer concrete rewrites. Match genre (blog, fiction, letter).\n\n"
        "Their request:\n{text}"
    ),
    Audience.BUSINESS: (
        "[Business mode] Workplace / product / ops context. Be concise, "
        "action-oriented, and stakeholder-friendly. Prefer bullets, next "
        "steps, and clear owners. Avoid engineering jargon unless needed.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DESIGN: (
        "[Design mode] Visual / UX craft. Talk hierarchy, spacing, type, "
        "contrast, and usability. Be concrete about UI changes; avoid pure "
        "code dumps unless they ask for implementation.\n\n"
        "Their request:\n{text}"
    ),
    Audience.JOB: (
        "[Career mode] Job search / resume / interview. Be practical and "
        "specific: stronger bullets, clearer stories, honest framing. "
        "No fabricated experience.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TEACHER: (
        "[Teacher mode] Education / lesson design. Learning goals, clear "
        "activities, age-appropriate language, and assessment ideas.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DATA: (
        "[Data mode] Analysis and evidence. Prefer clear methods, assumptions, "
        "and how to read the numbers. Formulas and query sketches OK; explain "
        "results in plain language too.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FOUNDER: (
        "[Founder mode] Early-stage product / company. Be direct about "
        "MVP scope, users, risks, and next experiments. Practical over "
        "corporate fluff.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LEGAL: (
        "[Legal-aware mode] Documents or rights language. Explain in plain "
        "English, flag risks and ambiguities, suggest questions for a real "
        "lawyer. You are not their attorney; this is not legal advice. "
        "Do not invent case law.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PARENT: (
        "[Parent mode] Family / kid-friendly. Warm, simple language; age-aware "
        "explanations; avoid scary or overly technical detail unless asked.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MARKETING: (
        "[Marketing mode] Campaigns, messaging, channels. Clear audience, "
        "hook, CTA, and measurable goals. Concrete copy options when useful.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SALES: (
        "[Sales mode] Pipeline and conversations. Practical talk tracks, "
        "objection handling, and next steps. No sleazy pressure tactics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FINANCE: (
        "[Finance mode] Numbers, books, budgets. Show assumptions, be precise "
        "with terms, and explain results plainly. Not personalized financial advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PRODUCT: (
        "[Product mode] Roadmaps, specs, user problems. Prioritize ruthlessly; "
        "write crisp requirements and acceptance criteria.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SUPPORT: (
        "[Support mode] Customer help. Empathetic, step-by-step troubleshooting, "
        "clear resolutions, reusable reply drafts.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SCIENCE: (
        "[Science mode] Experimental / STEM reasoning. Methods, units, controls, "
        "and careful claims. Show work on problems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LANGUAGE: (
        "[Language mode] Learning or translation. Correct gently, explain why, "
        "offer natural alternatives. Preserve meaning when translating.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CREATIVE: (
        "[Creative mode] Music, film, photo, storytelling craft. Concrete "
        "artistic suggestions; respect the person's voice and vision.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HEALTH: (
        "[Health-aware mode] General wellness information only. Clear, careful "
        "language; encourage professional care for medical decisions. "
        "Not a diagnosis or treatment plan.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NONPROFIT: (
        "[Nonprofit mode] Mission, donors, grants, volunteers. Practical and "
        "impact-oriented; respectful of constrained resources.\n\n"
        "Their request:\n{text}"
    ),
    Audience.POLICY: (
        "[Policy mode] Civic / public policy. Structured briefs, options, "
        "tradeoffs, and plain-language explanations for constituents.\n\n"
        "Their request:\n{text}"
    ),
    Audience.REAL_ESTATE: (
        "[Real estate mode] Homes, leases, deals. Practical steps, clear terms, "
        "and local-process awareness. Not a substitute for a licensed agent or attorney.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TRAVEL: (
        "[Travel mode] Itineraries and logistics. Realistic timing, packing, "
        "and options by budget; note that rules and prices change.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COOKING: (
        "[Cooking mode] Recipes and kitchen craft. Clear steps, quantities, "
        "substitutions, and timing. Food safety when relevant.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GAMING: (
        "[Gaming mode] Play, design, or stream. Practical builds, systems, "
        "and fun—respect spoilers if they matter.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SPORTS: (
        "[Sports mode] Training and competition. Clear plans, recovery, "
        "and technique cues. Not medical advice for injuries.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HR: (
        "[HR mode] People operations. Fair, policy-aware, professional "
        "templates. Not legal advice for employment disputes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.JOURNALISM: (
        "[Journalism mode] Reporting craft. Clear ledes, sourcing hygiene, "
        "and accuracy. Do not invent quotes or facts.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ACCESSIBILITY: (
        "[Accessibility mode] Inclusive design and WCAG-minded guidance. "
        "Concrete fixes for real users with disabilities.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ENGINEERING: (
        "[Engineering mode] Physical / systems engineering. Units, constraints, "
        "safety margins, and drawings—not pure software Ultracode unless asked.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SECURITY: (
        "[Security mode] Defensive security. Practical hardening and risk "
        "reduction. Refuse help that is clearly for attacking systems you "
        "do not own.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HOSPITALITY: (
        "[Hospitality mode] Guests, menus, service. Warm, operational, "
        "and quality-focused.\n\n"
        "Their request:\n{text}"
    ),
    Audience.EVENTS: (
        "[Events mode] Logistics and run-of-show. Timelines, vendors, "
        "and guest experience checklists.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FASHION: (
        "[Fashion mode] Style and garments. Concrete outfit or collection "
        "suggestions; respect body and budget constraints they mention.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DIY: (
        "[DIY mode] Home and hand tools. Step-by-step safety-first repairs "
        "and builds. Warn when a pro is required.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ENVIRONMENT: (
        "[Environment mode] Climate and sustainability. Evidence-aware, "
        "practical actions, no greenwashing hype.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SPIRITUAL: (
        "[Spiritual mode] Reflective and respectful of diverse traditions. "
        "Supportive, not coercive; no pretending to be clergy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SENIOR: (
        "[Senior mode] Aging and elder care. Clear, patient language; "
        "practical support for seniors and caregivers.\n\n"
        "Their request:\n{text}"
    ),
    Audience.AUTOMOTIVE: (
        "[Automotive mode] Cars and maintenance. Diagnostic steps and "
        "safety; say when a shop visit is needed.\n\n"
        "Their request:\n{text}"
    ),
    Audience.AGRICULTURE: (
        "[Agriculture mode] Crops, soil, livestock. Practical seasonal "
        "guidance; local rules and climate vary.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MUSIC: (
        "[Music mode] Songwriting and production. Theory when useful, "
        "concrete musical suggestions, respect their style.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PHOTOGRAPHY: (
        "[Photography mode] Photography craft. Exposure, composition, and editing tips.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FILM: (
        "[Film mode] Film/directing craft. Clear structure for scenes and production.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PODCAST: (
        "[Podcast mode] Podcast production. Outlines, interviews, and episode structure.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ARCHITECTURE: (
        "[Architecture mode] Architectural thinking. Space, structure, and drawings (not sealed plans).\n\n"
        "Their request:\n{text}"
    ),
    Audience.INTERIOR: (
        "[Interior mode] Interior design. Layouts, materials, and mood with real constraints.\n\n"
        "Their request:\n{text}"
    ),
    Audience.INSURANCE: (
        "[Insurance mode] Insurance concepts in plain English. Not a substitute for a licensed agent.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TAX: (
        "[Tax-aware] Tax concepts and checklists. Not tax advice; verify with a professional.\n\n"
        "Their request:\n{text}"
    ),
    Audience.INVESTING: (
        "[Investing mode] Investing education only. Not personalized investment advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CRYPTO: (
        "[Crypto mode] Crypto literacy with risk callouts. Not financial advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RETAIL: (
        "[Retail mode] Retail operations. Inventory, merchandising, and shop floor practicality.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ECOMMERCE: (
        "[Ecommerce mode] Online commerce. Listings, carts, and fulfillment basics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LOGISTICS: (
        "[Logistics mode] Logistics and supply chain. Practical routing and ops tradeoffs.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MANUFACTURING: (
        "[Manufacturing mode] Manufacturing process. Throughput, quality, and production practicality.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CONSTRUCTION: (
        "[Construction mode] Construction planning. Safety-first steps; call pros when required.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ROBOTICS: (
        "[Robotics mode] Robotics systems. Sensing, control, and careful real-world constraints.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MATH: (
        "[Math mode] Mathematics. Show steps clearly; explain the why.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PHILOSOPHY: (
        "[Philosophy mode] Philosophy. Clarify claims, objections, and reasoning.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PETS: (
        "[Pets mode] Pet care tips. Not veterinary diagnosis or treatment.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CHILDCARE: (
        "[Childcare mode] Childcare practicalities. Safety-first, age-aware guidance.\n\n"
        "Their request:\n{text}"
    ),
    Audience.IMMIGRATION: (
        "[Immigration-aware] Immigration process literacy. Not legal advice; rules change by country.\n\n"
        "Their request:\n{text}"
    ),
    Audience.THERAPY: (
        "[Therapy-aware] Supportive, non-clinical emotional framing. Not therapy or crisis care; encourage professionals when needed.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LIBRARY: (
        "[Library mode] Library and research skills. Finding and evaluating sources.\n\n"
        "Their request:\n{text}"
    ),
    Audience.THEATER: (
        "[Theater mode] Theater craft. Scripts, staging, and rehearsal practicality.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DANCE: (
        "[Dance mode] Dance and choreography. Clear practice structure.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WEATHER: (
        "[Weather mode] Weather awareness. Prep tips; forecasts change.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ASTRONOMY: (
        "[Astronomy mode] Astronomy. Observation tips and clear explanations.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COMPLIANCE: (
        "[Compliance mode] Compliance frameworks. Checklists and careful language—not a certification.\n\n"
        "Their request:\n{text}"
    ),
    Audience.OPERATIONS: (
        "[Operations mode] Operations. SOPs, runbooks, and reliable delivery.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PROCUREMENT: (
        "[Procurement mode] Procurement. Vendor selection and purchasing hygiene.\n\n"
        "Their request:\n{text}"
    ),
    Audience.QUALITY: (
        "[QA mode] Quality assurance. Test plans and clear bug reports.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GROWTH: (
        "[Growth mode] Growth experiments. Metrics, loops, and honest measurement.\n\n"
        "Their request:\n{text}"
    ),
    Audience.UX_RESEARCH: (
        "[UX research mode] UX research. Methods, synthesis, and evidence-based insights.\n\n"
        "Their request:\n{text}"
    ),
    Audience.STATS: (
        "[Stats mode] Statistics. Methods and interpretation with assumptions stated.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GENEALOGY: (
        "[Genealogy mode] Genealogy research. Records, trees, and careful claims.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COLLECTING: (
        "[Collecting mode] Collecting. Care and cataloging; valuations are estimates only.\n\n"
        "Their request:\n{text}"
    ),
    Audience.OUTDOORS: (
        "[Outdoors mode] Outdoors adventure. Safety-first plans and gear notes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GARDENING: (
        "[Gardening mode] Gardening. Seasonal, practical plant care.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BAKING: (
        "[Baking mode] Baking. Precise steps, temperatures, and troubleshooting.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COFFEE: (
        "[Coffee mode] Coffee brewing. Ratios, technique, and tasting language.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WINE: (
        "[Wine mode] Wine appreciation. Pairing and tasting; drink responsibly.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BEER: (
        "[Beer mode] Beer styles and homebrew basics; local laws apply.\n\n"
        "Their request:\n{text}"
    ),
    Audience.AVIATION: (
        "[Aviation mode] Aviation context. Safety-first, clear procedures; regulations vary by country.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MARITIME: (
        "[Maritime mode] Maritime context. Seamanship and port ops; local rules apply.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ENERGY: (
        "[Energy mode] Energy systems. Grid, generation, and efficiency with practical tradeoffs.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TELECOM: (
        "[Telecom mode] Telecom and connectivity. Clear network troubleshooting and planning language.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MEDIA: (
        "[Media mode] Media production and distribution. Formats, audiences, and platform constraints.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PR: (
        "[PR mode] Public relations. Clear messaging, stakeholder care, no invented quotes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SOCIAL_WORK: (
        "[Social work mode] Social-work oriented support. Practical resources language; not clinical or legal practice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ACCOUNTING: (
        "[Accounting mode] Accounting hygiene. Ledgers, reconciliations, clear assumptions—not a CPA substitute.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ACTING: (
        "[Acting mode] Acting craft. Character, monologue, and rehearsal practicality.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COMEDY: (
        "[Comedy mode] Comedy writing. Structure, premises, and punch-ups; respect taste boundaries they set.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WOODWORKING: (
        "[Woodworking mode] Woodworking. Joinery, tools, and shop safety step-by-step.\n\n"
        "Their request:\n{text}"
    ),
    Audience.METALWORKING: (
        "[Metalwork mode] Metalworking. Fabrication and safety-first process notes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ELECTRONICS: (
        "[Electronics mode] Electronics hobby/pro. Circuits, components, and careful power safety.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PRINTING_3D: (
        "[3D printing mode] 3D printing. Slicer settings, materials, and failure troubleshooting.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SEWING: (
        "[Sewing mode] Sewing. Patterns, stitches, and alterations with clear steps.\n\n"
        "Their request:\n{text}"
    ),
    Audience.KNITTING: (
        "[Knitting mode] Knitting/crochet-adjacent fiber craft. Gauge, patterns, and fixes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CHESS: (
        "[Chess mode] Chess. Openings, tactics, and clear analysis without engine spam unless asked.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TABLETOP: (
        "[Tabletop mode] Tabletop gaming. Rules clarity, campaign ideas, and session prep.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ANIME: (
        "[Anime mode] Anime/manga culture. Recommendations and analysis; spoiler-aware.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COMICS: (
        "[Comics mode] Comics craft. Panels, pacing, and sequential storytelling.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SCUBA: (
        "[Scuba mode] Scuba diving. Planning and safety; follow certification agency rules.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CYCLING: (
        "[Cycling mode] Cycling. Fit, training, and maintenance practicality.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RUNNING: (
        "[Running mode] Running training. Plans, form cues, and recovery notes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MARTIAL_ARTS: (
        "[Martial arts mode] Martial arts. Technique and training structure; respect safety and school culture.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NUTRITION: (
        "[Nutrition mode] Nutrition education. Patterns and macros; not medical diet therapy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PRODUCTIVITY: (
        "[Productivity mode] Productivity systems. Prioritization, focus, and sustainable habits.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PKM: (
        "[PKM mode] Personal knowledge management. Notes, links, and retrieval systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DEVOPS: (
        "[DevOps mode] DevOps. Pipelines, infra-as-code, and operational reliability.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CLOUD: (
        "[Cloud mode] Cloud architecture. Services, cost, and reliability tradeoffs.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NETWORKING: (
        "[Networking mode] Computer networking. Routing, troubleshooting, and clear diagrams in words.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DATABASE: (
        "[Database mode] Databases. Schema design, queries, and performance hygiene.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MOBILE: (
        "[Mobile mode] Mobile apps. Platform constraints, UX, and release practicality.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WEBDEV: (
        "[Web mode] Web development. Frontend/backend structure and practical implementation.\n\n"
        "Their request:\n{text}"
    ),
    Audience.EMBEDDED: (
        "[Embedded mode] Embedded systems. MCUs, timing, and resource constraints.\n\n"
        "Their request:\n{text}"
    ),
    Audience.IOT: (
        "[IoT mode] IoT systems. Devices, connectivity, and fleet ops basics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ARVR: (
        "[AR/VR mode] AR/VR. Spatial UX, comfort, and platform constraints.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FREELANCE: (
        "[Freelance mode] Freelancing. Pricing, clients, and scope control.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CONSULTING: (
        "[Consulting mode] Consulting craft. Structured recommendations and stakeholder-ready deliverables.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COACHING: (
        "[Coaching mode] Coaching-style support. Powerful questions and action plans—not clinical therapy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SPEAKING: (
        "[Speaking mode] Public speaking. Structure, slides, and delivery notes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RELATIONSHIPS: (
        "[Relationships mode] Relationship communication skills. Respectful, non-manipulative; not couples therapy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DATING: (
        "[Dating mode] Dating practicalities. Profiles and plans; respectful, no manipulation.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HISTORY: (
        "[History mode] History. Context and sources; do not invent primary documents.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GEOGRAPHY: (
        "[Geography mode] Geography. Places, regions, and spatial relationships clearly explained.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CHEMISTRY: (
        "[Chemistry mode] Chemistry. Equations and lab-minded safety notes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BIOLOGY: (
        "[Biology mode] Biology. Mechanisms and careful claims; no medical diagnosis.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PHYSICS: (
        "[Physics mode] Physics. Models, units, and worked reasoning.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MEDICINE: (
        "[Medicine-aware] Medical literacy only. Not diagnosis or treatment; urge professional care.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NURSING: (
        "[Nursing mode] Nursing-oriented education. Workflows and teaching; not clinical orders.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PHARMACY: (
        "[Pharmacy-aware] Pharmacy education. Mechanisms and counseling language; not prescribing.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DENTAL: (
        "[Dental-aware] Dental literacy. Not a substitute for a dentist.\n\n"
        "Their request:\n{text}"
    ),
    Audience.VETERINARY: (
        "[Vet-aware] Veterinary literacy. Not a substitute for a veterinarian.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MILITARY: (
        "[Military mode] Military topics with care. Educational only; refuse unlawful/harmful ops help.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FIRE: (
        "[Fire safety mode] Fire safety education. Prevention and emergency basics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.POLICE: (
        "[Public safety mode] Public-safety process literacy. Lawful, careful; not tactical wrongdoing help.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GEOLOGY: (
        "[Geology mode] Geology. Earth materials and processes with clear explanations.\n\n"
        "Their request:\n{text}"
    ),
    Audience.OCEAN: (
        "[Ocean mode] Ocean and marine systems. Clear science and coastal practicality.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ARCHAEOLOGY: (
        "[Archaeology mode] Archaeology. Methods and context; no looting guidance.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LINGUISTICS: (
        "[Linguistics mode] Linguistics. Structure of language, analysis, and clear examples.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FITNESS: (
        "[Fitness mode] Gym and general fitness. Progressive overload, form cues, and sustainable programming—not medical advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.YOGA: (
        "[Yoga mode] Yoga practice. Alignment, breath, and sequences; respect bodies and modifications.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CLIMBING: (
        "[Climbing mode] Climbing and bouldering. Technique, beta, training, and safety—not reckless risk.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GOLF: (
        "[Golf mode] Golf. Swing thoughts, practice plans, and course management—keep it practical.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FISHING: (
        "[Fishing mode] Fishing. Tackle, technique, and seasons; respect local regulations.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SWIMMING: (
        "[Swimming mode] Swimming. Stroke technique, sets, and open-water tips.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SKIING: (
        "[Ski/snow mode] Skiing and snowboarding. Technique and mountain safety first.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MOTORCYCLE: (
        "[Motorcycle mode] Motorcycles. Riding skills, gear, and maintenance—safety first.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DRONE: (
        "[Drone mode] Drones and UAS. Flight craft and legal/safety basics; no reckless ops.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GAME_DEV: (
        "[Game dev mode] Game development. Systems, loops, balance, and engines—implementation-minded.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ANIMATION: (
        "[Animation mode] Animation craft. Timing, arcs, and pipeline steps for 2D/3D.\n\n"
        "Their request:\n{text}"
    ),
    Audience.POETRY: (
        "[Poetry mode] Poetry. Form, image, sound, and revision—respect the poet’s voice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MAKEUP: (
        "[Makeup mode] Makeup. Looks and technique with inclusive, practical tips.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HAIR: (
        "[Hair mode] Hair care and styling. Cuts, products, and techniques for real hair types.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SKINCARE: (
        "[Skincare mode] Skincare education. Routines and ingredients; not dermatology or prescriptions.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WEDDING: (
        "[Wedding mode] Wedding planning. Timeline, budget, vendors, and guest experience.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PREGNANCY: (
        "[Pregnancy-aware] Pregnancy literacy only. Not medical advice; urge professional care for health questions.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SLEEP: (
        "[Sleep mode] Sleep hygiene and schedules. Not a substitute for clinical sleep medicine.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FIRST_AID: (
        "[First aid mode] First-aid education. Emergency basics; call emergency services for real incidents.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PUBLIC_HEALTH: (
        "[Public health mode] Public health. Prevention, systems, and population-level thinking—careful claims.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ML_AI: (
        "[ML/AI mode] Machine learning and AI systems. Methods, evaluation, and careful claims—implementation-minded.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SRE: (
        "[SRE mode] Site reliability engineering. SLOs, error budgets, incidents, and toil reduction.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SYSTEM_DESIGN: (
        "[System design mode] System design. Capacity, tradeoffs, and clear architecture diagrams in words.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TECH_WRITING: (
        "[Tech writing mode] Technical writing. Clear docs, examples, and audience-aware structure.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PROJECT_MGMT: (
        "[PM mode] Project management. Scope, risks, timelines, and stakeholder updates.\n\n"
        "Their request:\n{text}"
    ),
    Audience.AGILE: (
        "[Agile mode] Agile ways of working. Scrum/Kanban that reduce waste, not cargo-cult ceremonies.\n\n"
        "Their request:\n{text}"
    ),
    Audience.REMOTE_WORK: (
        "[Remote work mode] Remote and hybrid work. Async norms, tools, and sustainable focus.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CONTENT_CREATOR: (
        "[Creator mode] Content creation. Hooks, formats, and publishing systems—authentic, not spammy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SEO: (
        "[SEO mode] SEO. Technical and content tactics with ethical, sustainable practices.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BRAND: (
        "[Brand mode] Brand strategy. Positioning, voice, and identity systems that stay coherent.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NEGOTIATION: (
        "[Negotiation mode] Negotiation. Prep, BATNA, and fair outcomes—no manipulative dark patterns.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PATENT: (
        "[IP/patent-aware] IP and patent literacy only. Not legal advice; urge a qualified attorney for filings.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HOMESCHOOL: (
        "[Homeschool mode] Homeschooling. Plans, curricula, and age-appropriate activities.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TEST_PREP: (
        "[Test prep mode] Standardized test prep. Strategy, practice plans, and calm execution.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BARTENDING: (
        "[Bartending mode] Bartending and cocktails. Recipes and technique; drink responsibly.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TEA: (
        "[Tea mode] Tea. Types, brew parameters, and tasting notes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BBQ: (
        "[BBQ mode] BBQ and smoking. Fire management, temps, and rest—food safety included.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BEEKEEPING: (
        "[Beekeeping mode] Beekeeping. Hive management and seasonal care; safety around stings.\n\n"
        "Their request:\n{text}"
    ),
    Audience.AQUARIUM: (
        "[Aquarium mode] Aquariums. Water chemistry, stocking, and maintenance.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BIRDING: (
        "[Birding mode] Birding. Identification, habitats, and ethical observation.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HORSES: (
        "[Equestrian mode] Horses and equestrian care. Riding and stable management; safety first.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SURVIVAL: (
        "[Survival mode] Outdoor survival and bushcraft. Safety, ethics, and leave-no-trace—no illegal activity.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SMART_HOME: (
        "[Smart home mode] Smart home automation. Reliable setups and privacy-minded defaults.\n\n"
        "Their request:\n{text}"
    ),
    Audience.AUDIO_HIFI: (
        "[Hi-fi mode] Hi-fi audio. Speakers, sources, and room acoustics—practical listening upgrades.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WATCHES: (
        "[Watches mode] Watches. Movements, care, and collecting literacy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.JEWELRY: (
        "[Jewelry mode] Jewelry craft and selection. Materials, sizing, and care.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CERAMICS: (
        "[Ceramics mode] Ceramics. Throwing, handbuilding, glazes, and kiln safety.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CALLIGRAPHY: (
        "[Calligraphy mode] Calligraphy. Letterforms, tools, and deliberate practice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LEGO: (
        "[LEGO mode] LEGO and brick builds. Techniques, MOCs, and sorting systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HAM_RADIO: (
        "[Ham radio mode] Amateur (ham) radio. Licensing, antennas, and on-air etiquette; follow local law.\n\n"
        "Their request:\n{text}"
    ),
    Audience.QUANT: (
        "[Quant mode] Quantitative research literacy. Methods and risk—not trading advice or guaranteed alpha.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FRANCHISE: (
        "[Franchise mode] Franchising. Evaluation and operations literacy—not a substitute for legal/financial counsel.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RESTAURANT: (
        "[Restaurant mode] Restaurant operations. Menu, labor, service, and food safety basics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PROPERTY_MGMT: (
        "[Property mgmt mode] Property management. Tenants, maintenance, and operations—not legal advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PLUMBING: (
        "[Plumbing mode] Plumbing DIY literacy. Safety and when to call a licensed pro.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ELECTRICAL_TRADE: (
        "[Electrical trade mode] Electrical trade literacy. Safety first; licensed work for mains and code issues.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HVAC: (
        "[HVAC mode] HVAC systems. Comfort, filters, and when to call a tech.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MOVING: (
        "[Moving mode] Moving house. Packing, logistics, and settling-in checklists.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DECLUTTER: (
        "[Declutter mode] Decluttering and organizing. Practical systems without shame or extremes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DIGITAL_NOMAD: (
        "[Nomad mode] Digital nomad logistics. Visas awareness, connectivity, and sustainable travel work.\n\n"
        "Their request:\n{text}"
    ),
    Audience.EXPAT: (
        "[Expat mode] Expat living. Relocation literacy; not immigration legal advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NEURODIVERSITY: (
        "[Neurodiversity mode] Neurodiversity-aware practical support. Accommodations and systems; not diagnosis or therapy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DISABILITY: (
        "[Disability-aware] Disability-aware practical help. Access, accommodations, and dignity-first language.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PHYSICAL_THERAPY: (
        "[PT-aware] Physical therapy literacy. Movement education only; not clinical prescription.\n\n"
        "Their request:\n{text}"
    ),
    Audience.OPTOMETRY: (
        "[Vision-aware] Vision and eye-care literacy. Not a substitute for an optometrist or ophthalmologist.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GENETICS: (
        "[Genetics mode] Genetics. Inheritance and molecular basics; careful claims, no clinical genetic counseling.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BIOTECH: (
        "[Biotech mode] Biotech concepts and industry literacy. Careful, non-harmful educational framing.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SPACEFLIGHT: (
        "[Spaceflight mode] Spaceflight and rocketry concepts. Orbits, vehicles, and mission literacy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.URBAN_PLANNING: (
        "[Urban planning mode] Urban planning. Zoning, transit, housing, and public space tradeoffs.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DJ: (
        "[DJ mode] DJing. Mixing technique, crates, and gig craft.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GUITAR: (
        "[Guitar mode] Guitar. Technique, songs, and practice systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PIANO: (
        "[Piano mode] Piano. Technique, repertoire, and practice plans.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SINGING: (
        "[Singing mode] Singing. Breath, resonance, and repertoire—protect the voice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.IMPROV: (
        "[Improv mode] Improv. Yes-and, scene work, and playful collaboration.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FACILITATION: (
        "[Facilitation mode] Facilitation. Workshops and meetings with clear outcomes and inclusive process.\n\n"
        "Their request:\n{text}"
    ),
    Audience.UNION: (
        "[Labor/union mode] Labor and unions literacy. Lawful, educational workplace organizing context.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CAMPAIGN: (
        "[Campaign mode] Political and issue campaigns. Civic craft and field ops; lawful and non-violent.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COCKTAILS: (
        "[Mixology mode] Mixology. Classic formulas and modern twists; drink responsibly. (Prefer bartending for pro bar ops.)\n\n"
        "Their request:\n{text}"
    ),
    Audience.FERMENTATION: (
        "[Fermentation mode] Food fermentation. Process and food-safety minded steps.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FORAGING: (
        "[Foraging mode] Foraging. Identification caution and ethics; never eat unknowns.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MYCOLOGY: (
        "[Mycology mode] Mycology. Fungi biology and cultivation literacy; careful ID for wild mushrooms.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PERMACULTURE: (
        "[Permaculture mode] Permaculture design. Zones, stacking functions, and resilient food systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TINY_HOME: (
        "[Tiny home mode] Tiny homes and small-space living. Layout, systems, and realistic constraints.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HOME_THEATER: (
        "[Home theater mode] Home theater. Screens, audio, and room setup for great movies.\n\n"
        "Their request:\n{text}"
    ),
    Audience.STREAMING: (
        "[Streaming mode] Live streaming. Tech setup, scenes, and community growth.\n\n"
        "Their request:\n{text}"
    ),
    Audience.OPEN_SOURCE: (
        "[Open source mode] Open source. Contributing, licensing awareness, and healthy community norms.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DOCUMENTATION_SITE: (
        "[Docs site mode] Documentation sites. Information architecture, versioning, and search-friendly structure.\n\n"
        "Their request:\n{text}"
    ),
    Audience.OBSERVABILITY: (
        "[Observability mode] Observability. Metrics, logs, traces, and actionable alerts.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PLATFORM_ENG: (
        "[Platform eng mode] Platform engineering. Internal platforms, paved roads, and developer experience.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PRODUCT_MARKETING: (
        "[PMM mode] Product marketing. Launches, positioning, and enablement.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COPYWRITING: (
        "[Copywriting mode] Copywriting. Clear, conversion-aware prose that stays human.\n\n"
        "Their request:\n{text}"
    ),
    Audience.AFFILIATE: (
        "[Affiliate mode] Affiliate marketing. Systems and disclosure-minded ethics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.AMAZON_FBA: (
        "[Amazon FBA mode] Amazon FBA and marketplace ops. Listings, inventory, and seller practicalities.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ETSY: (
        "[Etsy mode] Etsy and handmade shops. Listings, photos, and small-shop ops.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GRANT_WRITING: (
        "[Grant writing mode] Grant writing. Clear need, plan, budget narrative, and outcomes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BOARD_GOVERNANCE: (
        "[Board mode] Board governance. Agendas, fiduciary duty awareness, and healthy oversight.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HIGHER_ED: (
        "[Higher ed mode] Higher education. Admissions, campus life, and academic systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SPECIAL_ED: (
        "[Special ed mode] Special education supports. Inclusive practices; not a substitute for formal evaluation.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ESL: (
        "[ESL/EFL mode] ESL/EFL teaching and learning. Clear models and patient practice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MEETING: (
        "[Meeting mode] Meetings. Agendas, notes, and decision hygiene so time is not wasted.\n\n"
        "Their request:\n{text}"
    ),
    Audience.OKRS: (
        "[OKR mode] OKRs. Ambitious objectives and measurable key results without theater.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CHANGE_MGMT: (
        "[Change mgmt mode] Change management. Stakeholder maps, communication, and adoption.\n\n"
        "Their request:\n{text}"
    ),
    Audience.VC: (
        "[VC/angel mode] Venture and angel literacy. Process and frameworks—not investment advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CROWDFUNDING: (
        "[Crowdfunding mode] Crowdfunding. Campaigns, rewards, and backer communication.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DISASTER_PREP: (
        "[Disaster prep mode] Disaster preparedness. Household plans and kits; follow official guidance in real events.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HUMANITARIAN: (
        "[Humanitarian mode] Humanitarian aid literacy. Ethical framing; no operational harm guidance.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LOCAL_GOV: (
        "[Local gov mode] Local government. Civic process, services, and public meetings literacy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HOUSING: (
        "[Housing mode] Housing systems literacy. Affordability and tenant rights awareness—not legal advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FOOD_SECURITY: (
        "[Food security mode] Food security. Access, systems, and community programs literacy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ZERO_WASTE: (
        "[Zero waste mode] Zero waste and waste reduction. Practical steps without purity culture.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COMPOSTING: (
        "[Composting mode] Composting. Browns/greens, bins, and troubleshooting.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SOLAR_HOME: (
        "[Home solar mode] Home solar and storage. Sizing literacy and questions to ask installers.\n\n"
        "Their request:\n{text}"
    ),
    Audience.EV: (
        "[EV mode] Electric vehicles. Charging, range, and ownership practicalities.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MOTORSPORTS: (
        "[Motorsports mode] Motorsports. Track craft and performance—legal, safe framing only.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SKATEBOARDING: (
        "[Skate mode] Skateboarding. Tricks, setup, and safety gear.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SURFING: (
        "[Surfing mode] Surfing. Wave sense, boards, and ocean safety.\n\n"
        "Their request:\n{text}"
    ),
    Audience.KAYAKING: (
        "[Kayak mode] Kayaking and paddling. Skills and water safety.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ROWING: (
        "[Rowing mode] Rowing. Technique, ergs, and boat craft.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TRIATHLON: (
        "[Triathlon mode] Triathlon training. Swim-bike-run balance and race logistics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.POWERLIFTING: (
        "[Powerlifting mode] Powerlifting. Squat/bench/deadlift programming and meet prep.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BODYBUILDING: (
        "[Bodybuilding mode] Bodybuilding. Hypertrophy and contest-prep literacy—not medical advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CALISTHENICS: (
        "[Calisthenics mode] Calisthenics. Bodyweight progressions and skill work.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PARKOUR: (
        "[Parkour mode] Parkour and freerunning. Safe progressions only; respect spaces and laws.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TENNIS: (
        "[Tennis mode] Tennis. Technique, strategy, and practice plans.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BASKETBALL: (
        "[Basketball mode] Basketball. Skills, plays, and training.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SOCCER: (
        "[Soccer mode] Soccer (football). Skills, tactics, and training.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BASEBALL: (
        "[Baseball mode] Baseball. Hitting, pitching, and fielding craft.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HOCKEY: (
        "[Hockey mode] Ice hockey. Skating, skills, and systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.VOLLEYBALL: (
        "[Volleyball mode] Volleyball. Skills and team systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BOXING: (
        "[Boxing mode] Boxing. Technique and conditioning; safety and sportsmanship first.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WRESTLING: (
        "[Wrestling mode] Wrestling. Technique and training structure; respect rules and safety.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FENCING: (
        "[Fencing mode] Fencing. Blade work and footwork for foil/epee/sabre.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ARCHERY: (
        "[Archery mode] Archery. Form, equipment, and safe practice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SAILING: (
        "[Sailing mode] Sailing. Seamanship, points of sail, and safety on the water.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HIKING: (
        "[Hiking mode] Hiking. Trails, gear, navigation, and leave-no-trace.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CAMPING: (
        "[Camping mode] Camping. Sites, gear, and comfortable nights outdoors.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BACKPACKING: (
        "[Backpacking mode] Backpacking. Multi-day routes, pack weight, and wilderness systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CROSSFIT: (
        "[CrossFit mode] CrossFit-style training. WODs, scaling, and form-minded intensity.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PILATES: (
        "[Pilates mode] Pilates. Core control, breath, and mat/reformer sequences.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GYMNASTICS: (
        "[Gymnastics mode] Gymnastics. Skill progressions and strength; spot and safety first.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ICE_SKATING: (
        "[Ice skating mode] Ice skating. Edges, freestyle, and rink basics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.OLYMPIC_LIFTING: (
        "[Olympic lifting mode] Olympic weightlifting. Snatch and clean & jerk technique and programming.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DRUMS: (
        "[Drums mode] Drums. Technique, grooves, and practice systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BASS: (
        "[Bass mode] Bass guitar. Lines, groove, and technique.\n\n"
        "Their request:\n{text}"
    ),
    Audience.VIOLIN: (
        "[Violin mode] Violin and bowed strings. Technique and deliberate practice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MUSIC_PRODUCTION: (
        "[Music production mode] Music production. DAWs, arrangement, and mix craft.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SOUND_DESIGN: (
        "[Sound design mode] Sound design. Synthesis, SFX, and sonic texture for media/games.\n\n"
        "Their request:\n{text}"
    ),
    Audience.VOICEOVER: (
        "[Voiceover mode] Voiceover. Scripts, delivery, and home-booth practicality.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SCREENWRITING: (
        "[Screenwriting mode] Screenwriting. Structure, scenes, and industry-aware formatting—no invented market claims.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NOVEL: (
        "[Novel mode] Novel writing. Structure, character, and revision for long-form fiction.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BLOGGING: (
        "[Blogging mode] Blogging. Posts, voice, SEO-aware structure, and publishing cadence.\n\n"
        "Their request:\n{text}"
    ),
    Audience.JOURNALING: (
        "[Journaling mode] Journaling. Prompts and reflective systems without therapy claims.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TRANSLATION: (
        "[Translation mode] Translation. Preserve meaning and register; note ambiguity.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SIGN_LANGUAGE: (
        "[Sign language mode] Sign language learning support. Respect Deaf culture; not a substitute for qualified instruction.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CROCHET: (
        "[Crochet mode] Crochet. Stitches, patterns, and project planning.\n\n"
        "Their request:\n{text}"
    ),
    Audience.EMBROIDERY: (
        "[Embroidery mode] Embroidery. Stitches, transfer, and finishing.\n\n"
        "Their request:\n{text}"
    ),
    Audience.QUILTING: (
        "[Quilting mode] Quilting. Blocks, piecing, and finishing.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COSPLAY: (
        "[Cosplay mode] Cosplay. Costumes, props, and con logistics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MAGIC_TRICKS: (
        "[Magic mode] Magic performance. Sleight of hand and presentation—no real-world fraud help.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MODEL_BUILDING: (
        "[Model building mode] Scale model building. Kits, techniques, and finishing.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LANDSCAPING: (
        "[Landscaping mode] Landscaping. Planting, hardscape, and outdoor design practicalities.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ROOFING: (
        "[Roofing mode] Roofing literacy. Safety first; licensed work for heights and structural issues.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PAINTING_TRADE: (
        "[House painting mode] House painting. Prep, products, and clean finishes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FLOORING: (
        "[Flooring mode] Flooring. Materials, install basics, and care.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CARPENTRY: (
        "[Carpentry mode] Carpentry. Framing, trim, and practical site work—safety first.\n\n"
        "Their request:\n{text}"
    ),
    Audience.APPLIANCE_REPAIR: (
        "[Appliance mode] Appliance repair literacy. Safety first; unplug and know when to call a tech.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PEST_CONTROL: (
        "[Pest control mode] Pest control literacy. Safe exclusion and when to call a pro; careful with chemicals.\n\n"
        "Their request:\n{text}"
    ),
    Audience.AUTO_BODY: (
        "[Auto body mode] Auto body. Dent, paint, and finish literacy—shop safety.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DOG_TRAINING: (
        "[Dog training mode] Dog training. Force-free, clear, welfare-minded methods.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CAT_CARE: (
        "[Cat care mode] Cat care. Behavior, enrichment, and basic health literacy—not a vet.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CHICKENS: (
        "[Chickens mode] Backyard chickens. Coops, care, and egg practicalities.\n\n"
        "Their request:\n{text}"
    ),
    Audience.REPTILES: (
        "[Reptiles mode] Reptile husbandry. Enclosures, heat, and welfare—not a vet.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CAREGIVING: (
        "[Caregiving mode] Caregiving. Practical systems and self-care for caregivers—not clinical orders.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CHRONIC_ILLNESS: (
        "[Chronic illness-aware] Chronic illness literacy and living strategies. Not diagnosis or treatment.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MASSAGE: (
        "[Massage mode] Massage and bodywork literacy. Relaxation and technique education—not clinical PT.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MENTAL_FITNESS: (
        "[Mental fitness mode] Mental fitness habits. Stress skills and routines—not clinical therapy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FERTILITY: (
        "[Fertility-aware] Fertility literacy only. Not medical advice; urge qualified care.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LACTATION: (
        "[Lactation-aware] Lactation and infant feeding literacy. Not a substitute for IBCLC/medical care.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PSYCHOLOGY: (
        "[Psychology mode] Psychology education. Concepts and research literacy—not therapy or diagnosis.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NEUROSCIENCE: (
        "[Neuroscience mode] Neuroscience. Brain systems carefully explained; no clinical claims.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ECONOMICS: (
        "[Economics mode] Economics. Incentives, models, and tradeoffs with clear assumptions.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SOCIOLOGY: (
        "[Sociology mode] Sociology. Social structures and research literacy; careful claims.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ANTHROPOLOGY: (
        "[Anthropology mode] Anthropology. Culture and fieldwork literacy; respectful comparison.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MATERIALS_SCIENCE: (
        "[Materials mode] Materials science. Properties, processing, and selection tradeoffs.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ECOLOGY: (
        "[Ecology mode] Ecology. Ecosystems, interactions, and conservation literacy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PERSONAL_FINANCE: (
        "[Personal finance mode] Personal finance literacy. Budgets and systems—not personalized financial advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RETIREMENT: (
        "[Retirement mode] Retirement planning literacy. Accounts and horizons—not investment advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ESTATE_PLANNING: (
        "[Estate-aware] Estate planning literacy only. Not legal advice; urge a qualified attorney.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SIDE_HUSTLE: (
        "[Side hustle mode] Side hustles. Realistic math, time, and risk—no get-rich-quick.\n\n"
        "Their request:\n{text}"
    ),
    Audience.REAL_ESTATE_INVESTING: (
        "[RE investing mode] Real estate investing literacy. Numbers and process—not investment advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.IMPORT_EXPORT: (
        "[Import/export mode] Import/export literacy. Docs, duties, and logistics basics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.INVENTORY: (
        "[Inventory mode] Inventory management. SKUs, counts, and replenishment systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DATA_ENGINEERING: (
        "[Data eng mode] Data engineering. Pipelines, warehouses, and data quality.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SPREADSHEETS: (
        "[Spreadsheet mode] Spreadsheets. Formulas, models, and clean structure.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NOCODE: (
        "[No-code mode] No-code and low-code. Builders, automations, and realistic limits.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WORDPRESS: (
        "[WordPress mode] WordPress. Themes, plugins, and site ops.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PRIVACY: (
        "[Privacy mode] Privacy hygiene. Data minimization and practical protections—not crime evasion.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PROMPT_ENG: (
        "[Prompt eng mode] Prompt engineering. Clear prompts, evals, and safe use of LLMs.\n\n"
        "Their request:\n{text}"
    ),
    Audience.KUBERNETES: (
        "[Kubernetes mode] Kubernetes. Workloads, networking, and cluster ops.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GRAPHICS_PROG: (
        "[Graphics prog mode] Graphics programming. Shaders, pipelines, and real-time rendering.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COMPILER: (
        "[Compiler mode] Compilers and language implementation. Parsing, IR, and codegen concepts.\n\n"
        "Their request:\n{text}"
    ),
    Audience.API_DESIGN: (
        "[API design mode] API design. Resources, versioning, and developer-friendly contracts.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FRONTEND: (
        "[Frontend mode] Frontend engineering. Components, state, and accessible UI implementation.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BACKEND: (
        "[Backend mode] Backend engineering. Services, data paths, and reliability basics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PARENTING_TEENS: (
        "[Teen parenting mode] Parenting teens. Respect, boundaries, and practical communication—not clinical care.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ADOPTION: (
        "[Adoption-aware] Adoption process literacy. Careful, respectful; not legal advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DIVORCE: (
        "[Divorce-aware] Divorce and separation logistics literacy. Not legal advice; urge qualified counsel.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GRIEF: (
        "[Grief-aware] Grief-aware supportive language. Not therapy; encourage professional/community support.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MINIMALISM: (
        "[Minimalism mode] Minimalism. Enough-ness and intentional ownership without purity culture.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LUXURY: (
        "[Luxury mode] Luxury goods literacy. Quality signals and care—without snobbery or status games.\n\n"
        "Their request:\n{text}"
    ),
    Audience.THRIFTING: (
        "[Thrifting mode] Thrifting and secondhand. Finding, evaluating, and upcycling.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ROAD_TRIP: (
        "[Road trip mode] Road trips. Routes, packing, and realistic drive days.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CRUISE: (
        "[Cruise mode] Cruises. Planning, cabins, and ship-life practicalities.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FOOD_TRAVEL: (
        "[Food travel mode] Food travel. Markets, reservations, and eating well away from home.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TUTORING: (
        "[Tutoring mode] Tutoring. Diagnose gaps, scaffold, and build independence.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CURRICULUM: (
        "[Curriculum mode] Curriculum design. Scope, sequence, and assessment alignment.\n\n"
        "Their request:\n{text}"
    ),
    Audience.EARLY_CHILDHOOD: (
        "[Early childhood mode] Early childhood. Play-based learning and care routines for young kids.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MONTESSORI: (
        "[Montessori mode] Montessori. Prepared environment, materials, and child-led work.\n\n"
        "Their request:\n{text}"
    ),
    Audience.EDTECH: (
        "[EdTech mode] EdTech. Learning products, classroom tech, and pedagogy-aware design.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NEIGHBORHOOD: (
        "[Neighborhood mode] Neighborhood and community. Local action, associations, and neighborly systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.VOLUNTEERING: (
        "[Volunteering mode] Volunteering. Matching skills to real needs and sustainable commitment.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FUNDRAISING_EVENTS: (
        "[Fundraising events mode] Fundraising events. Logistics, asks, and donor experience.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PHOTOGRAPHY_EDITING: (
        "[Photo editing mode] Photo editing. Lightroom/Photoshop craft and color workflow.\n\n"
        "Their request:\n{text}"
    ),
    Audience.VIDEO_EDITING: (
        "[Video editing mode] Video editing. Cuts, sound, color, and delivery formats.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PODCAST_EDITING: (
        "[Podcast editing mode] Podcast editing. Cleanup, assembly, and loudness targets.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NEWSLETTER: (
        "[Newsletter mode] Newsletters. Subject lines, cadence, and issue structure people open.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COMMUNITY_MGMT: (
        "[Community mode] Community management. Norms, moderation, and healthy growth.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CUSTOMER_SUCCESS: (
        "[CS mode] Customer success. Onboarding, health scores, and expansion—ethically.\n\n"
        "Their request:\n{text}"
    ),
    Audience.REVENUE_OPS: (
        "[RevOps mode] Revenue operations. Funnel systems, tooling, and clean data.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PEOPLE_OPS: (
        "[People ops mode] People ops. Systems for hiring, onboarding, and culture ops—fair and lawful framing.\n\n"
        "Their request:\n{text}"
    ),
    Audience.OFFICE_ADMIN: (
        "[Office admin mode] Office administration. Calendars, travel, vendors, and operational glue.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RESEARCH_METHODS: (
        "[Research methods mode] Research methods. Study design, sampling, and careful inference.\n\n"
        "Their request:\n{text}"
    ),
    Audience.STATISTICS_APPLIED: (
        "[Applied stats mode] Applied statistics. Real datasets, assumptions, and clear reporting.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CLIMATE_ACTION: (
        "[Climate action mode] Climate action. Personal and civic steps grounded in evidence.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RECYCLING: (
        "[Recycling mode] Recycling. Local rules, contamination, and better defaults.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WATER_CONSERVATION: (
        "[Water conservation mode] Water conservation. Practical savings at home and landscape.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HOME_SECURITY: (
        "[Home security mode] Home security. Locks, lighting, and cameras—sensible, lawful hardening.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CYBER_HYGIENE: (
        "[Cyber hygiene mode] Everyday cyber hygiene. Passwords, updates, and phishing sense—not offensive hacking.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PASSWORD_SECURITY: (
        "[Password mode] Password and passkey hygiene. Managers, uniqueness, and recovery.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BROWSER_EXT: (
        "[Browser extension mode] Browser extensions. Useful tools and permission caution.\n\n"
        "Their request:\n{text}"
    ),
    Audience.EMAIL_PRODUCTIVITY: (
        "[Email productivity mode] Email productivity. Inbox systems, templates, and triage.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NOTE_TAKING: (
        "[Note-taking mode] Note-taking. Capture, structure, and retrieval that sticks.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SPEED_READING: (
        "[Speed reading mode] Speed reading carefully. Pace with comprehension; no miracle claims.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DEBATE: (
        "[Debate mode] Debate. Structure, evidence, and fair rebuttal—not bad-faith tactics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PUBLIC_POLICY_ANALYSIS: (
        "[Policy analysis mode] Policy analysis. Options, criteria, and tradeoffs in plain language.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MAPS_GIS: (
        "[GIS/maps mode] GIS and maps. Layers, projections, and spatial analysis basics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CARTOGRAPHY: (
        "[Cartography mode] Cartography. Visual hierarchy and readable map design.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ASTROPHOTOGRAPHY: (
        "[Astrophotography mode] Astrophotography. Capture, tracking, and stacking basics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.METEOROLOGY_HOBBY: (
        "[Weather hobby mode] Hobby meteorology. Forecast literacy and storm safety—follow official warnings.\n\n"
        "Their request:\n{text}"
    ),
    Audience.AMATEUR_ASTRONOMY: (
        "[Amateur astronomy mode] Amateur astronomy. Scopes, observing, and sky literacy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BOARD_GAMES: (
        "[Board games mode] Board games. Rules teaching, strategy, and design notes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PUZZLES: (
        "[Puzzles mode] Puzzles. Solving strategies and puzzle construction basics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RUBIKS: (
        "[Cubing mode] Speedcubing. Methods, algorithms, and practice structure.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ORIGAMI: (
        "[Origami mode] Origami. Diagrams, bases, and folding design.\n\n"
        "Their request:\n{text}"
    ),
    Audience.KNIFE_SKILLS: (
        "[Knife skills mode] Kitchen knife skills. Technique and safety first.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MEAL_PREP: (
        "[Meal prep mode] Meal prep. Batch cooking, storage, and weekly systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.KETO: (
        "[Keto-aware] Ketogenic pattern literacy. Not medical diet therapy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.VEGAN_COOKING: (
        "[Vegan cooking mode] Vegan cooking. Flavorful plant-based meals and swaps.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GLUTEN_FREE: (
        "[Gluten-free mode] Gluten-free cooking. Swaps and cross-contact awareness—not medical advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SOUS_VIDE: (
        "[Sous vide mode] Sous vide. Temps, times, and finishing sears safely.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SMOKING_MEAT: (
        "[Smoking meat mode] Meat smoking. Low-and-slow temps, wood, and rest—food safety included.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PICKLEBALL: (
        "[Pickleball mode] Pickleball. Technique, strategy, and kitchen rules.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BADMINTON: (
        "[Badminton mode] Badminton. Strokes, footwork, and doubles strategy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TABLE_TENNIS: (
        "[Table tennis mode] Table tennis / ping pong. Spin, serve, and footwork.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RUGBY: (
        "[Rugby mode] Rugby. Skills, systems, and safe contact technique.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CRICKET: (
        "[Cricket mode] Cricket. Batting, bowling, and fielding craft.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SOFTBALL: (
        "[Softball mode] Softball. Pitching, hitting, and defense.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LACROSSE: (
        "[Lacrosse mode] Lacrosse. Stick skills, systems, and conditioning.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WATER_POLO: (
        "[Water polo mode] Water polo. Ball skills, treading, and team play.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DIVING_SPORT: (
        "[Springboard diving mode] Competitive diving. Board work, entries, and safe progressions.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SYNCHRONIZED_SWIM: (
        "[Artistic swim mode] Artistic / synchronized swimming. Routines, figures, and timing.\n\n"
        "Their request:\n{text}"
    ),
    Audience.EQUESTRIAN_SPORT: (
        "[Equestrian sport mode] Equestrian sport. Dressage, jumping, and eventing craft—horse welfare first.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ESPORTS: (
        "[Esports mode] Esports. Competitive play, practice structure, and team craft—not gambling.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SPEEDRUNNING: (
        "[Speedrun mode] Speedrunning. Routes, splits, and optimization craft.\n\n"
        "Their request:\n{text}"
    ),
    Audience.YOGA_THERAPY: (
        "[Yoga therapy-aware] Therapeutic yoga literacy. Not medical treatment; urge qualified care for conditions.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MOBILITY: (
        "[Mobility mode] Mobility training. Joint ranges and soft-tissue care routines.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BREATHWORK: (
        "[Breathwork mode] Breathwork. Safe practices; avoid extreme protocols without guidance.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SPANISH: (
        "[Spanish mode] Spanish language learning and usage. Natural phrasing and gentle correction.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FRENCH: (
        "[French mode] French language learning and usage. Natural phrasing and gentle correction.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GERMAN: (
        "[German mode] German language learning and usage. Cases, word order, gentle correction.\n\n"
        "Their request:\n{text}"
    ),
    Audience.JAPANESE: (
        "[Japanese mode] Japanese language learning. Kana, kanji, and natural usage.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MANDARIN: (
        "[Mandarin mode] Mandarin Chinese learning. Tones, characters, and practical phrases.\n\n"
        "Their request:\n{text}"
    ),
    Audience.KOREAN: (
        "[Korean mode] Korean language learning. Hangul and natural usage.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ITALIAN: (
        "[Italian mode] Italian language learning and usage.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PORTUGUESE: (
        "[Portuguese mode] Portuguese language learning (PT/BR). Natural phrasing.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ARABIC: (
        "[Arabic mode] Arabic language learning. Script and dialect notes carefully.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HINDI: (
        "[Hindi mode] Hindi language learning. Devanagari and practical usage.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GREEK_LANG: (
        "[Greek language mode] Modern Greek language learning and usage.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LATIN: (
        "[Latin mode] Latin. Grammar, reading, and classical literacy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.UKULELE: (
        "[Ukulele mode] Ukulele. Chords, strumming, and songs.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SAXOPHONE: (
        "[Saxophone mode] Saxophone. Tone, reeds, and practice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TRUMPET: (
        "[Trumpet mode] Trumpet and brass. Embouchure, range, and practice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FLUTE: (
        "[Flute mode] Flute. Tone, breath, and articulation practice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HARMONICA: (
        "[Harmonica mode] Harmonica. Bends, positions, and blues/folk craft.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BANJO: (
        "[Banjo mode] Banjo. Rolls, clawhammer, and bluegrass/old-time styles.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MUSIC_THEORY: (
        "[Music theory mode] Music theory. Harmony, form, and ear training with clear examples.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GRAPHIC_DESIGN: (
        "[Graphic design mode] Graphic design. Layout, type, color, and systems—implementation-minded.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ILLUSTRATION: (
        "[Illustration mode] Illustration. Drawing craft, process, and visual storytelling.\n\n"
        "Their request:\n{text}"
    ),
    Audience.UX_WRITING: (
        "[UX writing mode] UX writing. Microcopy, empty states, and error text that help.\n\n"
        "Their request:\n{text}"
    ),
    Audience.STORYBOARD: (
        "[Storyboard mode] Storyboarding. Shot planning and visual sequence for film/animation.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COLOR_GRADING: (
        "[Color grading mode] Color grading. Looks, scopes, and finishing for film/photo.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LIGHTING_DESIGN: (
        "[Lighting design mode] Lighting design. Stage, film, or architectural light with intent.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COSTUME_DESIGN: (
        "[Costume mode] Costume design. Character through clothes; practical build notes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SET_DESIGN: (
        "[Set design mode] Set and production design. Spaces that support story and camera.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PASTRY: (
        "[Pastry mode] Pastry craft. Laminated doughs, creams, and precise technique.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BREAD: (
        "[Bread mode] Bread baking. Fermentation, shaping, and crumb.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CHOCOLATE: (
        "[Chocolate mode] Chocolate craft. Tempering, truffles, and cacao literacy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CHEESE: (
        "[Cheese mode] Cheese. Making, aging, and pairing literacy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CHARCUTERIE: (
        "[Charcuterie mode] Charcuterie. Cured meats with food-safety caution.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PRESERVING: (
        "[Preserving mode] Food preserving. Canning, pickling, jams—botulism-aware safety first.\n\n"
        "Their request:\n{text}"
    ),
    Audience.INDIAN_COOKING: (
        "[Indian cooking mode] Indian cooking. Spices, dals, breads, and regional techniques.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CHINESE_COOKING: (
        "[Chinese cooking mode] Chinese cooking. Wok heat, sauces, and regional styles.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MEXICAN_COOKING: (
        "[Mexican cooking mode] Mexican cooking. Salsas, masa, and regional dishes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ITALIAN_COOKING: (
        "[Italian cooking mode] Italian cooking. Pasta, risotto, and regional classics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.JAPANESE_COOKING: (
        "[Japanese cooking mode] Japanese cooking. Dashi, rice, and home washoku techniques.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BBQ_SAUCES: (
        "[BBQ sauce mode] BBQ sauces and rubs. Regional styles and balance.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COFFEE_ROASTING: (
        "[Coffee roasting mode] Coffee roasting. Curves, development, and tasting.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LATTE_ART: (
        "[Latte art mode] Latte art. Milk texture and pouring patterns.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HOUSEPLANTS: (
        "[Houseplants mode] Houseplants. Light, water, and troubleshooting.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HYDROPONICS: (
        "[Hydroponics mode] Hydroponics. Systems, nutrients, and plant health.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BONSAI: (
        "[Bonsai mode] Bonsai. Styling, wiring, and tree health carefully.\n\n"
        "Their request:\n{text}"
    ),
    Audience.AQUAPONICS: (
        "[Aquaponics mode] Aquaponics. Fish-plant balance and system cycling.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LAWN_CARE: (
        "[Lawn care mode] Lawn care. Soil, mowing, and seasonal programs.\n\n"
        "Their request:\n{text}"
    ),
    Audience.IRRIGATION: (
        "[Irrigation mode] Irrigation. Zones, timers, and efficient watering.\n\n"
        "Their request:\n{text}"
    ),
    Audience.POOL_CARE: (
        "[Pool care mode] Pool care. Chemistry, filtration, and seasonal maintenance.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FIREPLACE: (
        "[Fireplace mode] Fireplaces and wood stoves. Safe operation and maintenance.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DENTAL_HYGIENE: (
        "[Dental hygiene mode] Dental hygiene literacy. Home care; not a substitute for a dentist/hygienist.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PHARMACOLOGY: (
        "[Pharmacology-aware] Pharmacology education. Mechanisms only—not prescribing or dosing advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RADIOLOGY_LITERACY: (
        "[Radiology-aware] Medical imaging literacy only. Not diagnosis; urge qualified clinicians.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NUTRITION_SCIENCE: (
        "[Nutrition science mode] Nutrition science literacy. Evidence-aware; not medical diet therapy.\n\n"
        "Their request:\n{text}"
    ),
    Audience.EPIDEMIOLOGY: (
        "[Epidemiology mode] Epidemiology. Rates, study designs, and careful population claims.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BIOSTATISTICS: (
        "[Biostats mode] Biostatistics. Study stats carefully with clear assumptions.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BOOKKEEPING: (
        "[Bookkeeping mode] Bookkeeping. Day-to-day entries and reconciliations—not a CPA substitute.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PAYROLL: (
        "[Payroll mode] Payroll literacy. Runs and withholdings—not tax filing advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BILLING: (
        "[Billing mode] Billing operations. Invoices, AR, and polite collections hygiene.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PRICING: (
        "[Pricing mode] Pricing strategy. Packaging, willingness-to-pay, and experiments.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SALES_ENABLEMENT: (
        "[Sales enablement mode] Sales enablement. Collateral, training, and deal support.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PARTNERSHIPS: (
        "[Partnerships mode] Partnerships. Alliances, channels, and co-selling structures.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CUSTOMER_RESEARCH: (
        "[Customer research mode] Customer research. Discovery interviews and jobs-to-be-done.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ANALYTICS: (
        "[Analytics mode] Analytics. Measurement plans, funnels, and trustworthy metrics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.AB_TESTING: (
        "[A/B testing mode] A/B testing. Design, power, and honest interpretation.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RUST_LANG: (
        "[Rust mode] Rust programming. Ownership, lifetimes, and idiomatic systems code.\n\n"
        "Their request:\n{text}"
    ),
    Audience.GO_LANG: (
        "[Go mode] Go programming. Simple concurrent services and idioms.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PYTHON_DATA: (
        "[Python data mode] Python for data. pandas/numpy workflows and clear analysis.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SQL_ANALYTICS: (
        "[SQL analytics mode] Analytical SQL. Queries that answer questions cleanly.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TERRAFORM: (
        "[Terraform mode] Terraform. Modules, state, and plan/apply discipline.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ANSIBLE: (
        "[Ansible mode] Ansible. Playbooks, roles, and idempotent config.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CICD: (
        "[CI/CD mode] CI/CD. Pipelines, gates, and safe shipping.\n\n"
        "Their request:\n{text}"
    ),
    Audience.DOCKER: (
        "[Docker mode] Docker. Images, containers, and compose stacks.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LINUX_ADMIN: (
        "[Linux admin mode] Linux administration. Users, services, and server craft.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NETWORK_SECURITY: (
        "[Network security mode] Network security. Defensive hardening and monitoring—not attacks.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PENTEST_DEFENSE: (
        "[Defense/pentest-aware] Defensive security testing literacy. Ethical, authorized scope only; no attack recipes for real systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.THREAT_MODEL: (
        "[Threat modeling mode] Threat modeling. Assets, threats, and mitigations clearly.\n\n"
        "Their request:\n{text}"
    ),
    Audience.INCIDENT_RESPONSE: (
        "[Incident response mode] Incident response. Detect, contain, communicate, and learn.\n\n"
        "Their request:\n{text}"
    ),
    Audience.QA_TESTING: (
        "[QA testing mode] QA testing. Strategy, cases, and quality signals.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MOBILE_QA: (
        "[Mobile QA mode] Mobile QA. Device matrices, gestures, and release confidence.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ACCESSIBILITY_ENG: (
        "[A11y eng mode] Accessibility engineering. Semantics, focus, and WCAG-minded implementation.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PERFORMANCE_WEB: (
        "[Web performance mode] Web performance. Core Web Vitals and practical speed fixes.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SEO_TECHNICAL: (
        "[Technical SEO mode] Technical SEO. Crawl/index health and structured data.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TAX_PREP: (
        "[Tax prep mode] Tax prep literacy. Forms and organization—not filing advice or a preparer substitute.\n\n"
        "Their request:\n{text}"
    ),
    Audience.INSURANCE_CLAIMS: (
        "[Insurance claims mode] Insurance claims literacy. Process steps—not a broker or coverage guarantee.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CAR_BUYING: (
        "[Car buying mode] Car buying. Research, negotiate, and total cost of ownership.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HOME_BUYING: (
        "[Home buying mode] Home buying literacy. Offers, inspections, closing—not a realtor substitute.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RENTING: (
        "[Renting mode] Renting. Leases, roommates, and tenant practicalities—not legal advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COLLEGE_APPS: (
        "[College apps mode] College applications. Essays, lists, and process—honest framing only.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SCHOLARSHIPS: (
        "[Scholarships mode] Scholarships. Search, essays, and application logistics.\n\n"
        "Their request:\n{text}"
    ),
    Audience.STUDY_ABROAD: (
        "[Study abroad mode] Study abroad. Programs, packing, and cultural prep.\n\n"
        "Their request:\n{text}"
    ),
    Audience.INTERNSHIP: (
        "[Internship mode] Internships. Finding, applying, and performing well.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CAREER_CHANGE: (
        "[Career change mode] Career change. Transferable skills and honest narrative—not fabricated experience.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LINKEDIN: (
        "[LinkedIn mode] LinkedIn help. For messages and InMail: draft clear, "
        "warm, non-spammy replies the person can paste back; ask for the other "
        "party’s message if missing. For profile/headline: stay human and "
        "honest—no fabricated experience.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WHATSAPP: (
        "[WhatsApp mode] WhatsApp help. Draft natural, paste-ready replies for "
        "chats the person pastes in—match their tone, keep it short, never claim "
        "you sent anything. No WhatsApp Business API; they paste and send.\n\n"
        "Their request:\n{text}"
    ),
    Audience.NETWORKING_CAREER: (
        "[Career networking mode] Career networking. Warm intros and relationship craft—not sleaze.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HOA_LIVING: (
        "[HOA mode] HOA living. Rules, process, and neighborly problem-solving—not legal advice.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COOP_HOUSING: (
        "[Co-op housing mode] Housing co-ops. Shares, governance, and shared living practicalities.\n\n"
        "Their request:\n{text}"
    ),
    Audience.COMMUNITY_GARDEN: (
        "[Community garden mode] Community gardens. Shared plots, rules, and seasonal coordination.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MUTUAL_AID: (
        "[Mutual aid mode] Mutual aid. Neighbor help systems and sustainable care networks.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FISHKEEPING: (
        "[Fishkeeping mode] Fishkeeping. Husbandry, stocking, and water stability.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TERRARIUM: (
        "[Terrarium mode] Terrariums and vivariums. Closed ecosystems and plant/animal balance.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ANTKEEPING: (
        "[Antkeeping mode] Antkeeping. Formicariums and colony care ethically.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BEEKEEPING_ADVANCED: (
        "[Advanced beekeeping mode] Advanced beekeeping. Splits, queens, and seasonal management.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FOUNTAIN_PEN: (
        "[Fountain pen mode] Fountain pens. Nibs, inks, paper, and maintenance.\n\n"
        "Their request:\n{text}"
    ),
    Audience.STATIONERY: (
        "[Stationery mode] Stationery. Notebooks, pens, and paper systems.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MECHANICAL_KEYBOARD: (
        "[Mech keyboard mode] Mechanical keyboards. Switches, kits, and build craft.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PC_BUILDING: (
        "[PC building mode] PC building. Parts selection, thermals, and assembly.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HOME_LAB: (
        "[Homelab mode] Homelab. Self-hosted services and learning labs safely.\n\n"
        "Their request:\n{text}"
    ),
    Audience.THREE_D_MODELING: (
        "[3D modeling mode] 3D modeling. Meshes, CAD, and printable form.\n\n"
        "Their request:\n{text}"
    ),
    Audience.CNC: (
        "[CNC mode] CNC. Toolpaths, feeds/speeds, and machine safety.\n\n"
        "Their request:\n{text}"
    ),
    Audience.LASER_CUTTING: (
        "[Laser cutting mode] Laser cutting. Vectors, materials, and safe operation.\n\n"
        "Their request:\n{text}"
    ),
    Audience.RESIN_PRINTING: (
        "[Resin printing mode] Resin (SLA/DLP) printing. Settings, wash/cure, and chemical safety.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FILAMENT_PRINTING: (
        "[FDM printing mode] FDM/filament printing. Settings, materials, and quality.\n\n"
        "Their request:\n{text}"
    ),
    Audience.MEDITATION: (
        "[Meditation mode] Meditation. Practice structures across traditions without dogma.\n\n"
        "Their request:\n{text}"
    ),
    Audience.STOICISM: (
        "[Stoicism mode] Stoicism. Practical exercises and texts carefully—not toxic hardness.\n\n"
        "Their request:\n{text}"
    ),
    Audience.JOURNAL_PROMPTS: (
        "[Journal prompts mode] Journal prompts. Open reflection without therapy claims.\n\n"
        "Their request:\n{text}"
    ),
    Audience.HABIT_BUILDING: (
        "[Habit mode] Habit building. Tiny systems, cues, and recovery after slips.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TIME_BLOCKING: (
        "[Time blocking mode] Time blocking. Calendar systems that protect deep work.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SECOND_BRAIN: (
        "[Second brain mode] Second brain systems. Capture, organize, and retrieve for action.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PACKING: (
        "[Packing mode] Packing. Light, complete packing systems for trips.\n\n"
        "Their request:\n{text}"
    ),
    Audience.TRAVEL_PHOTOGRAPHY: (
        "[Travel photo mode] Travel photography. Stories on the move with light gear.\n\n"
        "Their request:\n{text}"
    ),
    Audience.SOLO_TRAVEL: (
        "[Solo travel mode] Solo travel. Independent trips with safety and enjoyment.\n\n"
        "Their request:\n{text}"
    ),
    Audience.FAMILY_TRAVEL: (
        "[Family travel mode] Family travel. Kids, logistics, and sane pacing.\n\n"
        "Their request:\n{text}"
    ),
    Audience.BUDGET_TRAVEL: (
        "[Budget travel mode] Budget travel. More trip for less money without misery.\n\n"
        "Their request:\n{text}"
    ),
    Audience.POINTS_MILES: (
        "[Points & miles mode] Points and miles. Loyalty programs carefully—no manufactured spend schemes that break rules.\n\n"
        "Their request:\n{text}"
    ),
    Audience.REAL_ESTATE_PHOTO: (
        "[RE photography mode] Real estate photography. Listing photos that show space honestly.\n\n"
        "Their request:\n{text}"
    ),
    Audience.STAGING: (
        "[Home staging mode] Home staging. Rooms that photograph and show well.\n\n"
        "Their request:\n{text}"
    ),
    Audience.INTERIOR_STYLING: (
        "[Interior styling mode] Interior styling. Vignettes, textiles, and finish layers.\n\n"
        "Their request:\n{text}"
    ),
    Audience.EVENT_PHOTOGRAPHY: (
        "[Event photo mode] Event photography. Candid craft and delivery workflows.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PORTRAIT_PHOTO: (
        "[Portrait photo mode] Portrait photography. Light, posing, and direction.\n\n"
        "Their request:\n{text}"
    ),
    Audience.STREET_PHOTO: (
        "[Street photo mode] Street photography. Public scenes with ethics and awareness.\n\n"
        "Their request:\n{text}"
    ),
    Audience.WILDLIFE_PHOTO: (
        "[Wildlife photo mode] Wildlife photography. Ethics, fieldcraft, and long glass.\n\n"
        "Their request:\n{text}"
    ),
    Audience.ASTRO_IMAGING_PROC: (
        "[Astro processing mode] Astrophotography processing. Stack, stretch, and finish carefully.\n\n"
        "Their request:\n{text}"
    ),
    Audience.PLAIN: (
        "The person is not looking for specialist jargon. Use short everyday "
        "words. Do not ask them to open a terminal, run commands, or read stack "
        "traces. If you change files, say what you did in plain language. "
        "If a technical word is unavoidable, define it once in one simple sentence.\n\n"
        "Their request:\n{text}"
    ),
}


def shape_prompt(prompt: str, *, audience: Audience | None = None) -> str:
    """Wrap the user's ask for the model; UI still shows the original text."""
    text = (prompt or "").strip()
    if not text:
        return text
    if audience is None:
        audience = detect_audience(text)
    tmpl = _SHAPE.get(audience) or _SHAPE[Audience.PLAIN]
    return tmpl.format(text=text)


# Back-compat alias used by older call sites / tests.
def shape_prompt_for_claude(prompt: str) -> str:
    return shape_prompt(prompt)


def shape_prompt_for_tool(tool_id: str, prompt: str) -> str:
    """Shape prompts for coding/chat helpers; pass through everything else."""
    tid = (tool_id or "").strip().lower()
    text = (prompt or "").strip()
    if tid in SHAPED_TOOLS:
        return shape_prompt(text)
    return text


_CHIP_LABELS: dict[Audience, str] = {
    Audience.CODE: "Ultracode",
    Audience.ACADEMIC: "Academic mode",
    Audience.STUDENT: "Student mode",
    Audience.WRITER: "Writer mode",
    Audience.BUSINESS: "Business mode",
    Audience.DESIGN: "Design mode",
    Audience.JOB: "Career mode",
    Audience.TEACHER: "Teacher mode",
    Audience.DATA: "Data mode",
    Audience.FOUNDER: "Founder mode",
    Audience.LEGAL: "Legal-aware",
    Audience.PARENT: "Parent mode",
    Audience.MARKETING: "Marketing mode",
    Audience.SALES: "Sales mode",
    Audience.FINANCE: "Finance mode",
    Audience.PRODUCT: "Product mode",
    Audience.SUPPORT: "Support mode",
    Audience.SCIENCE: "Science mode",
    Audience.LANGUAGE: "Language mode",
    Audience.CREATIVE: "Creative mode",
    Audience.HEALTH: "Health-aware",
    Audience.NONPROFIT: "Nonprofit mode",
    Audience.POLICY: "Policy mode",
    Audience.REAL_ESTATE: "Real estate",
    Audience.TRAVEL: "Travel mode",
    Audience.COOKING: "Cooking mode",
    Audience.GAMING: "Gaming mode",
    Audience.SPORTS: "Sports mode",
    Audience.HR: "HR mode",
    Audience.JOURNALISM: "Journalism mode",
    Audience.ACCESSIBILITY: "Accessibility",
    Audience.ENGINEERING: "Engineering mode",
    Audience.SECURITY: "Security mode",
    Audience.HOSPITALITY: "Hospitality mode",
    Audience.EVENTS: "Events mode",
    Audience.FASHION: "Fashion mode",
    Audience.DIY: "DIY mode",
    Audience.ENVIRONMENT: "Environment mode",
    Audience.SPIRITUAL: "Spiritual mode",
    Audience.SENIOR: "Senior mode",
    Audience.AUTOMOTIVE: "Automotive mode",
    Audience.AGRICULTURE: "Agriculture mode",
    Audience.MUSIC: "Music mode",
    Audience.PHOTOGRAPHY: "Photography mode",
    Audience.FILM: "Film mode",
    Audience.PODCAST: "Podcast mode",
    Audience.ARCHITECTURE: "Architecture mode",
    Audience.INTERIOR: "Interior mode",
    Audience.INSURANCE: "Insurance mode",
    Audience.TAX: "Tax-aware",
    Audience.INVESTING: "Investing mode",
    Audience.CRYPTO: "Crypto mode",
    Audience.RETAIL: "Retail mode",
    Audience.ECOMMERCE: "Ecommerce mode",
    Audience.LOGISTICS: "Logistics mode",
    Audience.MANUFACTURING: "Manufacturing mode",
    Audience.CONSTRUCTION: "Construction mode",
    Audience.ROBOTICS: "Robotics mode",
    Audience.MATH: "Math mode",
    Audience.PHILOSOPHY: "Philosophy mode",
    Audience.PETS: "Pets mode",
    Audience.CHILDCARE: "Childcare mode",
    Audience.IMMIGRATION: "Immigration-aware",
    Audience.THERAPY: "Therapy-aware",
    Audience.LIBRARY: "Library mode",
    Audience.THEATER: "Theater mode",
    Audience.DANCE: "Dance mode",
    Audience.WEATHER: "Weather mode",
    Audience.ASTRONOMY: "Astronomy mode",
    Audience.COMPLIANCE: "Compliance mode",
    Audience.OPERATIONS: "Operations mode",
    Audience.PROCUREMENT: "Procurement mode",
    Audience.QUALITY: "QA mode",
    Audience.GROWTH: "Growth mode",
    Audience.UX_RESEARCH: "UX research mode",
    Audience.STATS: "Stats mode",
    Audience.GENEALOGY: "Genealogy mode",
    Audience.COLLECTING: "Collecting mode",
    Audience.OUTDOORS: "Outdoors mode",
    Audience.GARDENING: "Gardening mode",
    Audience.BAKING: "Baking mode",
    Audience.COFFEE: "Coffee mode",
    Audience.WINE: "Wine mode",
    Audience.BEER: "Beer mode",
    Audience.AVIATION: "Aviation mode",
    Audience.MARITIME: "Maritime mode",
    Audience.ENERGY: "Energy mode",
    Audience.TELECOM: "Telecom mode",
    Audience.MEDIA: "Media mode",
    Audience.PR: "PR mode",
    Audience.SOCIAL_WORK: "Social work mode",
    Audience.ACCOUNTING: "Accounting mode",
    Audience.ACTING: "Acting mode",
    Audience.COMEDY: "Comedy mode",
    Audience.WOODWORKING: "Woodworking mode",
    Audience.METALWORKING: "Metalwork mode",
    Audience.ELECTRONICS: "Electronics mode",
    Audience.PRINTING_3D: "3D printing mode",
    Audience.SEWING: "Sewing mode",
    Audience.KNITTING: "Knitting mode",
    Audience.CHESS: "Chess mode",
    Audience.TABLETOP: "Tabletop mode",
    Audience.ANIME: "Anime mode",
    Audience.COMICS: "Comics mode",
    Audience.SCUBA: "Scuba mode",
    Audience.CYCLING: "Cycling mode",
    Audience.RUNNING: "Running mode",
    Audience.MARTIAL_ARTS: "Martial arts mode",
    Audience.NUTRITION: "Nutrition mode",
    Audience.PRODUCTIVITY: "Productivity mode",
    Audience.PKM: "PKM mode",
    Audience.DEVOPS: "DevOps mode",
    Audience.CLOUD: "Cloud mode",
    Audience.NETWORKING: "Networking mode",
    Audience.DATABASE: "Database mode",
    Audience.MOBILE: "Mobile mode",
    Audience.WEBDEV: "Web mode",
    Audience.EMBEDDED: "Embedded mode",
    Audience.IOT: "IoT mode",
    Audience.ARVR: "AR/VR mode",
    Audience.FREELANCE: "Freelance mode",
    Audience.CONSULTING: "Consulting mode",
    Audience.COACHING: "Coaching mode",
    Audience.SPEAKING: "Speaking mode",
    Audience.RELATIONSHIPS: "Relationships mode",
    Audience.DATING: "Dating mode",
    Audience.HISTORY: "History mode",
    Audience.GEOGRAPHY: "Geography mode",
    Audience.CHEMISTRY: "Chemistry mode",
    Audience.BIOLOGY: "Biology mode",
    Audience.PHYSICS: "Physics mode",
    Audience.MEDICINE: "Medicine-aware",
    Audience.NURSING: "Nursing mode",
    Audience.PHARMACY: "Pharmacy-aware",
    Audience.DENTAL: "Dental-aware",
    Audience.VETERINARY: "Vet-aware",
    Audience.MILITARY: "Military mode",
    Audience.FIRE: "Fire safety mode",
    Audience.POLICE: "Public safety mode",
    Audience.GEOLOGY: "Geology mode",
    Audience.OCEAN: "Ocean mode",
    Audience.ARCHAEOLOGY: "Archaeology mode",
    Audience.LINGUISTICS: "Linguistics mode",
    Audience.FITNESS: "Fitness mode",
    Audience.YOGA: "Yoga mode",
    Audience.CLIMBING: "Climbing mode",
    Audience.GOLF: "Golf mode",
    Audience.FISHING: "Fishing mode",
    Audience.SWIMMING: "Swimming mode",
    Audience.SKIING: "Ski/snow mode",
    Audience.MOTORCYCLE: "Motorcycle mode",
    Audience.DRONE: "Drone mode",
    Audience.GAME_DEV: "Game dev mode",
    Audience.ANIMATION: "Animation mode",
    Audience.POETRY: "Poetry mode",
    Audience.MAKEUP: "Makeup mode",
    Audience.HAIR: "Hair mode",
    Audience.SKINCARE: "Skincare mode",
    Audience.WEDDING: "Wedding mode",
    Audience.PREGNANCY: "Pregnancy-aware",
    Audience.SLEEP: "Sleep mode",
    Audience.FIRST_AID: "First aid mode",
    Audience.PUBLIC_HEALTH: "Public health mode",
    Audience.ML_AI: "ML/AI mode",
    Audience.SRE: "SRE mode",
    Audience.SYSTEM_DESIGN: "System design mode",
    Audience.TECH_WRITING: "Tech writing mode",
    Audience.PROJECT_MGMT: "PM mode",
    Audience.AGILE: "Agile mode",
    Audience.REMOTE_WORK: "Remote work mode",
    Audience.CONTENT_CREATOR: "Creator mode",
    Audience.SEO: "SEO mode",
    Audience.BRAND: "Brand mode",
    Audience.NEGOTIATION: "Negotiation mode",
    Audience.PATENT: "IP/patent-aware",
    Audience.HOMESCHOOL: "Homeschool mode",
    Audience.TEST_PREP: "Test prep mode",
    Audience.BARTENDING: "Bartending mode",
    Audience.TEA: "Tea mode",
    Audience.BBQ: "BBQ mode",
    Audience.BEEKEEPING: "Beekeeping mode",
    Audience.AQUARIUM: "Aquarium mode",
    Audience.BIRDING: "Birding mode",
    Audience.HORSES: "Equestrian mode",
    Audience.SURVIVAL: "Survival mode",
    Audience.SMART_HOME: "Smart home mode",
    Audience.AUDIO_HIFI: "Hi-fi mode",
    Audience.WATCHES: "Watches mode",
    Audience.JEWELRY: "Jewelry mode",
    Audience.CERAMICS: "Ceramics mode",
    Audience.CALLIGRAPHY: "Calligraphy mode",
    Audience.LEGO: "LEGO mode",
    Audience.HAM_RADIO: "Ham radio mode",
    Audience.QUANT: "Quant mode",
    Audience.FRANCHISE: "Franchise mode",
    Audience.RESTAURANT: "Restaurant mode",
    Audience.PROPERTY_MGMT: "Property mgmt mode",
    Audience.PLUMBING: "Plumbing mode",
    Audience.ELECTRICAL_TRADE: "Electrical trade mode",
    Audience.HVAC: "HVAC mode",
    Audience.MOVING: "Moving mode",
    Audience.DECLUTTER: "Declutter mode",
    Audience.DIGITAL_NOMAD: "Nomad mode",
    Audience.EXPAT: "Expat mode",
    Audience.NEURODIVERSITY: "Neurodiversity mode",
    Audience.DISABILITY: "Disability-aware",
    Audience.PHYSICAL_THERAPY: "PT-aware",
    Audience.OPTOMETRY: "Vision-aware",
    Audience.GENETICS: "Genetics mode",
    Audience.BIOTECH: "Biotech mode",
    Audience.SPACEFLIGHT: "Spaceflight mode",
    Audience.URBAN_PLANNING: "Urban planning mode",
    Audience.DJ: "DJ mode",
    Audience.GUITAR: "Guitar mode",
    Audience.PIANO: "Piano mode",
    Audience.SINGING: "Singing mode",
    Audience.IMPROV: "Improv mode",
    Audience.FACILITATION: "Facilitation mode",
    Audience.UNION: "Labor/union mode",
    Audience.CAMPAIGN: "Campaign mode",
    Audience.COCKTAILS: "Mixology mode",
    Audience.FERMENTATION: "Fermentation mode",
    Audience.FORAGING: "Foraging mode",
    Audience.MYCOLOGY: "Mycology mode",
    Audience.PERMACULTURE: "Permaculture mode",
    Audience.TINY_HOME: "Tiny home mode",
    Audience.HOME_THEATER: "Home theater mode",
    Audience.STREAMING: "Streaming mode",
    Audience.OPEN_SOURCE: "Open source mode",
    Audience.DOCUMENTATION_SITE: "Docs site mode",
    Audience.OBSERVABILITY: "Observability mode",
    Audience.PLATFORM_ENG: "Platform eng mode",
    Audience.PRODUCT_MARKETING: "PMM mode",
    Audience.COPYWRITING: "Copywriting mode",
    Audience.AFFILIATE: "Affiliate mode",
    Audience.AMAZON_FBA: "Amazon FBA mode",
    Audience.ETSY: "Etsy mode",
    Audience.GRANT_WRITING: "Grant writing mode",
    Audience.BOARD_GOVERNANCE: "Board mode",
    Audience.HIGHER_ED: "Higher ed mode",
    Audience.SPECIAL_ED: "Special ed mode",
    Audience.ESL: "ESL/EFL mode",
    Audience.MEETING: "Meeting mode",
    Audience.OKRS: "OKR mode",
    Audience.CHANGE_MGMT: "Change mgmt mode",
    Audience.VC: "VC/angel mode",
    Audience.CROWDFUNDING: "Crowdfunding mode",
    Audience.DISASTER_PREP: "Disaster prep mode",
    Audience.HUMANITARIAN: "Humanitarian mode",
    Audience.LOCAL_GOV: "Local gov mode",
    Audience.HOUSING: "Housing mode",
    Audience.FOOD_SECURITY: "Food security mode",
    Audience.ZERO_WASTE: "Zero waste mode",
    Audience.COMPOSTING: "Composting mode",
    Audience.SOLAR_HOME: "Home solar mode",
    Audience.EV: "EV mode",
    Audience.MOTORSPORTS: "Motorsports mode",
    Audience.SKATEBOARDING: "Skate mode",
    Audience.SURFING: "Surfing mode",
    Audience.KAYAKING: "Kayak mode",
    Audience.ROWING: "Rowing mode",
    Audience.TRIATHLON: "Triathlon mode",
    Audience.POWERLIFTING: "Powerlifting mode",
    Audience.BODYBUILDING: "Bodybuilding mode",
    Audience.CALISTHENICS: "Calisthenics mode",
    Audience.PARKOUR: "Parkour mode",
    Audience.TENNIS: "Tennis mode",
    Audience.BASKETBALL: "Basketball mode",
    Audience.SOCCER: "Soccer mode",
    Audience.BASEBALL: "Baseball mode",
    Audience.HOCKEY: "Hockey mode",
    Audience.VOLLEYBALL: "Volleyball mode",
    Audience.BOXING: "Boxing mode",
    Audience.WRESTLING: "Wrestling mode",
    Audience.FENCING: "Fencing mode",
    Audience.ARCHERY: "Archery mode",
    Audience.SAILING: "Sailing mode",
    Audience.HIKING: "Hiking mode",
    Audience.CAMPING: "Camping mode",
    Audience.BACKPACKING: "Backpacking mode",
    Audience.CROSSFIT: "CrossFit mode",
    Audience.PILATES: "Pilates mode",
    Audience.GYMNASTICS: "Gymnastics mode",
    Audience.ICE_SKATING: "Ice skating mode",
    Audience.OLYMPIC_LIFTING: "Olympic lifting mode",
    Audience.DRUMS: "Drums mode",
    Audience.BASS: "Bass mode",
    Audience.VIOLIN: "Violin mode",
    Audience.MUSIC_PRODUCTION: "Music production mode",
    Audience.SOUND_DESIGN: "Sound design mode",
    Audience.VOICEOVER: "Voiceover mode",
    Audience.SCREENWRITING: "Screenwriting mode",
    Audience.NOVEL: "Novel mode",
    Audience.BLOGGING: "Blogging mode",
    Audience.JOURNALING: "Journaling mode",
    Audience.TRANSLATION: "Translation mode",
    Audience.SIGN_LANGUAGE: "Sign language mode",
    Audience.CROCHET: "Crochet mode",
    Audience.EMBROIDERY: "Embroidery mode",
    Audience.QUILTING: "Quilting mode",
    Audience.COSPLAY: "Cosplay mode",
    Audience.MAGIC_TRICKS: "Magic mode",
    Audience.MODEL_BUILDING: "Model building mode",
    Audience.LANDSCAPING: "Landscaping mode",
    Audience.ROOFING: "Roofing mode",
    Audience.PAINTING_TRADE: "House painting mode",
    Audience.FLOORING: "Flooring mode",
    Audience.CARPENTRY: "Carpentry mode",
    Audience.APPLIANCE_REPAIR: "Appliance mode",
    Audience.PEST_CONTROL: "Pest control mode",
    Audience.AUTO_BODY: "Auto body mode",
    Audience.DOG_TRAINING: "Dog training mode",
    Audience.CAT_CARE: "Cat care mode",
    Audience.CHICKENS: "Chickens mode",
    Audience.REPTILES: "Reptiles mode",
    Audience.CAREGIVING: "Caregiving mode",
    Audience.CHRONIC_ILLNESS: "Chronic illness-aware",
    Audience.MASSAGE: "Massage mode",
    Audience.MENTAL_FITNESS: "Mental fitness mode",
    Audience.FERTILITY: "Fertility-aware",
    Audience.LACTATION: "Lactation-aware",
    Audience.PSYCHOLOGY: "Psychology mode",
    Audience.NEUROSCIENCE: "Neuroscience mode",
    Audience.ECONOMICS: "Economics mode",
    Audience.SOCIOLOGY: "Sociology mode",
    Audience.ANTHROPOLOGY: "Anthropology mode",
    Audience.MATERIALS_SCIENCE: "Materials mode",
    Audience.ECOLOGY: "Ecology mode",
    Audience.PERSONAL_FINANCE: "Personal finance mode",
    Audience.RETIREMENT: "Retirement mode",
    Audience.ESTATE_PLANNING: "Estate-aware",
    Audience.SIDE_HUSTLE: "Side hustle mode",
    Audience.REAL_ESTATE_INVESTING: "RE investing mode",
    Audience.IMPORT_EXPORT: "Import/export mode",
    Audience.INVENTORY: "Inventory mode",
    Audience.DATA_ENGINEERING: "Data eng mode",
    Audience.SPREADSHEETS: "Spreadsheet mode",
    Audience.NOCODE: "No-code mode",
    Audience.WORDPRESS: "WordPress mode",
    Audience.PRIVACY: "Privacy mode",
    Audience.PROMPT_ENG: "Prompt eng mode",
    Audience.KUBERNETES: "Kubernetes mode",
    Audience.GRAPHICS_PROG: "Graphics prog mode",
    Audience.COMPILER: "Compiler mode",
    Audience.API_DESIGN: "API design mode",
    Audience.FRONTEND: "Frontend mode",
    Audience.BACKEND: "Backend mode",
    Audience.PARENTING_TEENS: "Teen parenting mode",
    Audience.ADOPTION: "Adoption-aware",
    Audience.DIVORCE: "Divorce-aware",
    Audience.GRIEF: "Grief-aware",
    Audience.MINIMALISM: "Minimalism mode",
    Audience.LUXURY: "Luxury mode",
    Audience.THRIFTING: "Thrifting mode",
    Audience.ROAD_TRIP: "Road trip mode",
    Audience.CRUISE: "Cruise mode",
    Audience.FOOD_TRAVEL: "Food travel mode",
    Audience.TUTORING: "Tutoring mode",
    Audience.CURRICULUM: "Curriculum mode",
    Audience.EARLY_CHILDHOOD: "Early childhood mode",
    Audience.MONTESSORI: "Montessori mode",
    Audience.EDTECH: "EdTech mode",
    Audience.NEIGHBORHOOD: "Neighborhood mode",
    Audience.VOLUNTEERING: "Volunteering mode",
    Audience.FUNDRAISING_EVENTS: "Fundraising events mode",
    Audience.PHOTOGRAPHY_EDITING: "Photo editing mode",
    Audience.VIDEO_EDITING: "Video editing mode",
    Audience.PODCAST_EDITING: "Podcast editing mode",
    Audience.NEWSLETTER: "Newsletter mode",
    Audience.COMMUNITY_MGMT: "Community mode",
    Audience.CUSTOMER_SUCCESS: "CS mode",
    Audience.REVENUE_OPS: "RevOps mode",
    Audience.PEOPLE_OPS: "People ops mode",
    Audience.OFFICE_ADMIN: "Office admin mode",
    Audience.RESEARCH_METHODS: "Research methods mode",
    Audience.STATISTICS_APPLIED: "Applied stats mode",
    Audience.CLIMATE_ACTION: "Climate action mode",
    Audience.RECYCLING: "Recycling mode",
    Audience.WATER_CONSERVATION: "Water conservation mode",
    Audience.HOME_SECURITY: "Home security mode",
    Audience.CYBER_HYGIENE: "Cyber hygiene mode",
    Audience.PASSWORD_SECURITY: "Password mode",
    Audience.BROWSER_EXT: "Browser extension mode",
    Audience.EMAIL_PRODUCTIVITY: "Email productivity mode",
    Audience.NOTE_TAKING: "Note-taking mode",
    Audience.SPEED_READING: "Speed reading mode",
    Audience.DEBATE: "Debate mode",
    Audience.PUBLIC_POLICY_ANALYSIS: "Policy analysis mode",
    Audience.MAPS_GIS: "GIS/maps mode",
    Audience.CARTOGRAPHY: "Cartography mode",
    Audience.ASTROPHOTOGRAPHY: "Astrophotography mode",
    Audience.METEOROLOGY_HOBBY: "Weather hobby mode",
    Audience.AMATEUR_ASTRONOMY: "Amateur astronomy mode",
    Audience.BOARD_GAMES: "Board games mode",
    Audience.PUZZLES: "Puzzles mode",
    Audience.RUBIKS: "Cubing mode",
    Audience.ORIGAMI: "Origami mode",
    Audience.KNIFE_SKILLS: "Knife skills mode",
    Audience.MEAL_PREP: "Meal prep mode",
    Audience.KETO: "Keto-aware",
    Audience.VEGAN_COOKING: "Vegan cooking mode",
    Audience.GLUTEN_FREE: "Gluten-free mode",
    Audience.SOUS_VIDE: "Sous vide mode",
    Audience.SMOKING_MEAT: "Smoking meat mode",
    Audience.PICKLEBALL: "Pickleball mode",
    Audience.BADMINTON: "Badminton mode",
    Audience.TABLE_TENNIS: "Table tennis mode",
    Audience.RUGBY: "Rugby mode",
    Audience.CRICKET: "Cricket mode",
    Audience.SOFTBALL: "Softball mode",
    Audience.LACROSSE: "Lacrosse mode",
    Audience.WATER_POLO: "Water polo mode",
    Audience.DIVING_SPORT: "Springboard diving mode",
    Audience.SYNCHRONIZED_SWIM: "Artistic swim mode",
    Audience.EQUESTRIAN_SPORT: "Equestrian sport mode",
    Audience.ESPORTS: "Esports mode",
    Audience.SPEEDRUNNING: "Speedrun mode",
    Audience.YOGA_THERAPY: "Yoga therapy-aware",
    Audience.MOBILITY: "Mobility mode",
    Audience.BREATHWORK: "Breathwork mode",
    Audience.SPANISH: "Spanish mode",
    Audience.FRENCH: "French mode",
    Audience.GERMAN: "German mode",
    Audience.JAPANESE: "Japanese mode",
    Audience.MANDARIN: "Mandarin mode",
    Audience.KOREAN: "Korean mode",
    Audience.ITALIAN: "Italian mode",
    Audience.PORTUGUESE: "Portuguese mode",
    Audience.ARABIC: "Arabic mode",
    Audience.HINDI: "Hindi mode",
    Audience.GREEK_LANG: "Greek language mode",
    Audience.LATIN: "Latin mode",
    Audience.UKULELE: "Ukulele mode",
    Audience.SAXOPHONE: "Saxophone mode",
    Audience.TRUMPET: "Trumpet mode",
    Audience.FLUTE: "Flute mode",
    Audience.HARMONICA: "Harmonica mode",
    Audience.BANJO: "Banjo mode",
    Audience.MUSIC_THEORY: "Music theory mode",
    Audience.GRAPHIC_DESIGN: "Graphic design mode",
    Audience.ILLUSTRATION: "Illustration mode",
    Audience.UX_WRITING: "UX writing mode",
    Audience.STORYBOARD: "Storyboard mode",
    Audience.COLOR_GRADING: "Color grading mode",
    Audience.LIGHTING_DESIGN: "Lighting design mode",
    Audience.COSTUME_DESIGN: "Costume mode",
    Audience.SET_DESIGN: "Set design mode",
    Audience.PASTRY: "Pastry mode",
    Audience.BREAD: "Bread mode",
    Audience.CHOCOLATE: "Chocolate mode",
    Audience.CHEESE: "Cheese mode",
    Audience.CHARCUTERIE: "Charcuterie mode",
    Audience.PRESERVING: "Preserving mode",
    Audience.INDIAN_COOKING: "Indian cooking mode",
    Audience.CHINESE_COOKING: "Chinese cooking mode",
    Audience.MEXICAN_COOKING: "Mexican cooking mode",
    Audience.ITALIAN_COOKING: "Italian cooking mode",
    Audience.JAPANESE_COOKING: "Japanese cooking mode",
    Audience.BBQ_SAUCES: "BBQ sauce mode",
    Audience.COFFEE_ROASTING: "Coffee roasting mode",
    Audience.LATTE_ART: "Latte art mode",
    Audience.HOUSEPLANTS: "Houseplants mode",
    Audience.HYDROPONICS: "Hydroponics mode",
    Audience.BONSAI: "Bonsai mode",
    Audience.AQUAPONICS: "Aquaponics mode",
    Audience.LAWN_CARE: "Lawn care mode",
    Audience.IRRIGATION: "Irrigation mode",
    Audience.POOL_CARE: "Pool care mode",
    Audience.FIREPLACE: "Fireplace mode",
    Audience.DENTAL_HYGIENE: "Dental hygiene mode",
    Audience.PHARMACOLOGY: "Pharmacology-aware",
    Audience.RADIOLOGY_LITERACY: "Radiology-aware",
    Audience.NUTRITION_SCIENCE: "Nutrition science mode",
    Audience.EPIDEMIOLOGY: "Epidemiology mode",
    Audience.BIOSTATISTICS: "Biostats mode",
    Audience.BOOKKEEPING: "Bookkeeping mode",
    Audience.PAYROLL: "Payroll mode",
    Audience.BILLING: "Billing mode",
    Audience.PRICING: "Pricing mode",
    Audience.SALES_ENABLEMENT: "Sales enablement mode",
    Audience.PARTNERSHIPS: "Partnerships mode",
    Audience.CUSTOMER_RESEARCH: "Customer research mode",
    Audience.ANALYTICS: "Analytics mode",
    Audience.AB_TESTING: "A/B testing mode",
    Audience.RUST_LANG: "Rust mode",
    Audience.GO_LANG: "Go mode",
    Audience.PYTHON_DATA: "Python data mode",
    Audience.SQL_ANALYTICS: "SQL analytics mode",
    Audience.TERRAFORM: "Terraform mode",
    Audience.ANSIBLE: "Ansible mode",
    Audience.CICD: "CI/CD mode",
    Audience.DOCKER: "Docker mode",
    Audience.LINUX_ADMIN: "Linux admin mode",
    Audience.NETWORK_SECURITY: "Network security mode",
    Audience.PENTEST_DEFENSE: "Defense/pentest-aware",
    Audience.THREAT_MODEL: "Threat modeling mode",
    Audience.INCIDENT_RESPONSE: "Incident response mode",
    Audience.QA_TESTING: "QA testing mode",
    Audience.MOBILE_QA: "Mobile QA mode",
    Audience.ACCESSIBILITY_ENG: "A11y eng mode",
    Audience.PERFORMANCE_WEB: "Web performance mode",
    Audience.SEO_TECHNICAL: "Technical SEO mode",
    Audience.TAX_PREP: "Tax prep mode",
    Audience.INSURANCE_CLAIMS: "Insurance claims mode",
    Audience.CAR_BUYING: "Car buying mode",
    Audience.HOME_BUYING: "Home buying mode",
    Audience.RENTING: "Renting mode",
    Audience.COLLEGE_APPS: "College apps mode",
    Audience.SCHOLARSHIPS: "Scholarships mode",
    Audience.STUDY_ABROAD: "Study abroad mode",
    Audience.INTERNSHIP: "Internship mode",
    Audience.CAREER_CHANGE: "Career change mode",
    Audience.LINKEDIN: "LinkedIn mode",
    Audience.WHATSAPP: "WhatsApp mode",
    Audience.NETWORKING_CAREER: "Career networking mode",
    Audience.HOA_LIVING: "HOA mode",
    Audience.COOP_HOUSING: "Co-op housing mode",
    Audience.COMMUNITY_GARDEN: "Community garden mode",
    Audience.MUTUAL_AID: "Mutual aid mode",
    Audience.FISHKEEPING: "Fishkeeping mode",
    Audience.TERRARIUM: "Terrarium mode",
    Audience.ANTKEEPING: "Antkeeping mode",
    Audience.BEEKEEPING_ADVANCED: "Advanced beekeeping mode",
    Audience.FOUNTAIN_PEN: "Fountain pen mode",
    Audience.STATIONERY: "Stationery mode",
    Audience.MECHANICAL_KEYBOARD: "Mech keyboard mode",
    Audience.PC_BUILDING: "PC building mode",
    Audience.HOME_LAB: "Homelab mode",
    Audience.THREE_D_MODELING: "3D modeling mode",
    Audience.CNC: "CNC mode",
    Audience.LASER_CUTTING: "Laser cutting mode",
    Audience.RESIN_PRINTING: "Resin printing mode",
    Audience.FILAMENT_PRINTING: "FDM printing mode",
    Audience.MEDITATION: "Meditation mode",
    Audience.STOICISM: "Stoicism mode",
    Audience.JOURNAL_PROMPTS: "Journal prompts mode",
    Audience.HABIT_BUILDING: "Habit mode",
    Audience.TIME_BLOCKING: "Time blocking mode",
    Audience.SECOND_BRAIN: "Second brain mode",
    Audience.PACKING: "Packing mode",
    Audience.TRAVEL_PHOTOGRAPHY: "Travel photo mode",
    Audience.SOLO_TRAVEL: "Solo travel mode",
    Audience.FAMILY_TRAVEL: "Family travel mode",
    Audience.BUDGET_TRAVEL: "Budget travel mode",
    Audience.POINTS_MILES: "Points & miles mode",
    Audience.REAL_ESTATE_PHOTO: "RE photography mode",
    Audience.STAGING: "Home staging mode",
    Audience.INTERIOR_STYLING: "Interior styling mode",
    Audience.EVENT_PHOTOGRAPHY: "Event photo mode",
    Audience.PORTRAIT_PHOTO: "Portrait photo mode",
    Audience.STREET_PHOTO: "Street photo mode",
    Audience.WILDLIFE_PHOTO: "Wildlife photo mode",
    Audience.ASTRO_IMAGING_PROC: "Astro processing mode",
    Audience.PLAIN: "Keeping it simple",
}

_CHIP_HINTS: dict[Audience, str] = {
    Audience.CODE: "You sound like you know code — we’ll dig into the project and help make the change.",
    Audience.ACADEMIC: "Research / scholarly tone — careful structure, no fake citations.",
    Audience.STUDENT: "Learning mode — clear steps and explanations, not just the answer.",
    Audience.WRITER: "Prose and tone — rewrites, clarity, and voice.",
    Audience.BUSINESS: "Workplace clarity — concise, action-oriented, stakeholder-friendly.",
    Audience.DESIGN: "Visual / UX craft — hierarchy, spacing, usability.",
    Audience.JOB: "Career help — resumes, interviews, honest framing.",
    Audience.TEACHER: "Teaching tools — lessons, goals, age-appropriate activities.",
    Audience.DATA: "Numbers with context — methods, charts, plain-language takeaways.",
    Audience.FOUNDER: "Startup pace — MVP, users, experiments, less fluff.",
    Audience.LEGAL: "Plain-English rights language — not a substitute for a lawyer.",
    Audience.PARENT: "Family-friendly — simple, warm, age-aware.",
    Audience.MARKETING: "Campaigns and messaging — audience, hook, CTA, measurement.",
    Audience.SALES: "Pipeline conversations — discovery, objections, next steps.",
    Audience.FINANCE: "Books and budgets — clear assumptions (not personal financial advice).",
    Audience.PRODUCT: "Roadmaps and specs — prioritize ruthlessly, write crisp requirements.",
    Audience.SUPPORT: "Customer help — empathetic steps and reusable replies.",
    Audience.SCIENCE: "STEM / lab reasoning — methods, units, careful claims.",
    Audience.LANGUAGE: "Translate or learn — natural phrasing and gentle corrections.",
    Audience.CREATIVE: "Arts and media craft — suggestions that respect your vision.",
    Audience.HEALTH: "General wellness info only — not a diagnosis or treatment plan.",
    Audience.NONPROFIT: "Mission, donors, grants — practical impact under constraints.",
    Audience.POLICY: "Civic briefs — options, tradeoffs, plain language for the public.",
    Audience.REAL_ESTATE: "Homes and leases — practical steps (not a licensed agent).",
    Audience.TRAVEL: "Trips and logistics — realistic plans and packing.",
    Audience.COOKING: "Kitchen craft — clear recipes, swaps, and timing.",
    Audience.GAMING: "Play and game design — builds, systems, and fun.",
    Audience.SPORTS: "Training and competition — plans and technique (not medical care).",
    Audience.HR: "People ops — fair, professional, policy-aware templates.",
    Audience.JOURNALISM: "Reporting craft — accuracy first, no invented facts.",
    Audience.ACCESSIBILITY: "Inclusive design — concrete WCAG-minded fixes.",
    Audience.ENGINEERING: "Physical systems — units, constraints, safety margins.",
    Audience.SECURITY: "Defensive security — harden and reduce risk responsibly.",
    Audience.HOSPITALITY: "Guests and service — warm, operational, quality-focused.",
    Audience.EVENTS: "Run-of-show logistics — timelines, vendors, guest experience.",
    Audience.FASHION: "Style and garments — outfits and collections with real constraints.",
    Audience.DIY: "Home fixes and builds — safety-first steps.",
    Audience.ENVIRONMENT: "Climate and sustainability — practical, evidence-aware.",
    Audience.SPIRITUAL: "Reflective support — respectful of diverse traditions.",
    Audience.SENIOR: "Aging and caregivers — clear, patient, practical.",
    Audience.AUTOMOTIVE: "Cars and maintenance — diagnostics and when to see a shop.",
    Audience.AGRICULTURE: "Crops and livestock — seasonal, practical guidance.",
    Audience.MUSIC: "Songwriting and production — theory when useful, concrete musical ideas.",
    Audience.PHOTOGRAPHY: "Cameras and composition — settings, lighting, edits.",
    Audience.FILM: "Screen and cinema craft — shots, story, post.",
    Audience.PODCAST: "Episodes, guests, show notes, audio workflow.",
    Audience.ARCHITECTURE: "Buildings and space — plans, codes awareness.",
    Audience.INTERIOR: "Rooms, furniture, and atmosphere.",
    Audience.INSURANCE: "Coverage language in plain English (not a broker).",
    Audience.TAX: "Filing concepts — not a tax preparer or advice.",
    Audience.INVESTING: "Portfolios and markets — not investment advice.",
    Audience.CRYPTO: "Onchain basics carefully — high risk, no promises.",
    Audience.RETAIL: "Stores, inventory, merchandising.",
    Audience.ECOMMERCE: "Online stores, listings, conversion.",
    Audience.LOGISTICS: "Shipping, warehousing, supply chain.",
    Audience.MANUFACTURING: "Production, BOM, yield.",
    Audience.CONSTRUCTION: "Jobsite, bids, builds — safety first.",
    Audience.ROBOTICS: "Robots, sensors, autonomy.",
    Audience.MATH: "Show the work; teach the method.",
    Audience.PHILOSOPHY: "Careful arguments, not slogans.",
    Audience.PETS: "Animal care — not a substitute for a vet.",
    Audience.CHILDCARE: "Routines for little ones — safety first.",
    Audience.IMMIGRATION: "Process literacy — not legal advice.",
    Audience.THERAPY: "Supportive coping — not clinical care.",
    Audience.LIBRARY: "Research paths and sources.",
    Audience.THEATER: "Stage craft, scripts, rehearsal.",
    Audience.DANCE: "Movement, choreography, practice.",
    Audience.WEATHER: "Forecast literacy and prep.",
    Audience.ASTRONOMY: "Sky, scopes, cosmos.",
    Audience.COMPLIANCE: "Controls and audits — careful and accurate.",
    Audience.OPERATIONS: "Runbooks, process, reliability.",
    Audience.PROCUREMENT: "Vendors, RFPs, purchasing.",
    Audience.QUALITY: "Testing, defects, acceptance.",
    Audience.GROWTH: "Activation, retention, experiments.",
    Audience.UX_RESEARCH: "Interviews, synthesis, usability.",
    Audience.STATS: "Inference carefully; show assumptions.",
    Audience.GENEALOGY: "Family history research paths.",
    Audience.COLLECTING: "Collections and care — values vary.",
    Audience.OUTDOORS: "Trails, camping, gear — safety first.",
    Audience.GARDENING: "Plants, soil, seasons.",
    Audience.BAKING: "Dough, heat, timing — precise steps.",
    Audience.COFFEE: "Brew recipes and tasting notes.",
    Audience.WINE: "Pairing and tasting — drink responsibly.",
    Audience.BEER: "Styles and homebrew basics.",
    Audience.AVIATION: "Flights, aircraft, and air ops — safety first.",
    Audience.MARITIME: "Ships, ports, and sea ops.",
    Audience.ENERGY: "Power systems, renewables, grids.",
    Audience.TELECOM: "Networks, carriers, connectivity.",
    Audience.MEDIA: "Broadcast, publishing, platforms.",
    Audience.PR: "Reputation, press, crisis comms.",
    Audience.SOCIAL_WORK: "Case support — not clinical/legal practice.",
    Audience.ACCOUNTING: "Books, ledgers, GAAP-aware language.",
    Audience.ACTING: "Performance, monologues, character work.",
    Audience.COMEDY: "Jokes, bits, timing — punch up carefully.",
    Audience.WOODWORKING: "Joinery, tools, shop safety.",
    Audience.METALWORKING: "Welding, fab, machines — safety first.",
    Audience.ELECTRONICS: "Circuits, components, hobby builds.",
    Audience.PRINTING_3D: "Prints, slicers, materials.",
    Audience.SEWING: "Patterns, stitches, alterations.",
    Audience.KNITTING: "Yarn, gauges, patterns.",
    Audience.CHESS: "Openings, tactics, analysis.",
    Audience.TABLETOP: "TTRPG, boards, campaigns.",
    Audience.ANIME: "Anime/manga discussion and recs.",
    Audience.COMICS: "Panels, scripts, sequential art.",
    Audience.SCUBA: "Diving plans — certification rules matter.",
    Audience.CYCLING: "Bikes, routes, fit, training.",
    Audience.RUNNING: "Plans, form, races.",
    Audience.MARTIAL_ARTS: "Technique, training, respect dojo culture.",
    Audience.NUTRITION: "Food patterns — not medical diet therapy.",
    Audience.PRODUCTIVITY: "Systems, focus, prioritization.",
    Audience.PKM: "Notes, linking, personal knowledge.",
    Audience.DEVOPS: "CI/CD, infra as code, reliability.",
    Audience.CLOUD: "AWS/GCP/Azure architecture.",
    Audience.NETWORKING: "IP, routing, packets, Wi‑Fi.",
    Audience.DATABASE: "Schemas, queries, performance.",
    Audience.MOBILE: "iOS/Android apps and UX.",
    Audience.WEBDEV: "Frontend/backend web systems.",
    Audience.EMBEDDED: "Firmware, MCUs, constraints.",
    Audience.IOT: "Sensors, devices, fleets.",
    Audience.ARVR: "Spatial interfaces and headsets.",
    Audience.FREELANCE: "Rates, contracts, client work.",
    Audience.CONSULTING: "Engagements, decks, recommendations.",
    Audience.COACHING: "Questions that unlock action — not therapy.",
    Audience.SPEAKING: "Talks, slides, stage presence.",
    Audience.RELATIONSHIPS: "Communication skills — not therapy.",
    Audience.DATING: "Profiles and plans — respectful.",
    Audience.HISTORY: "Evidence-aware historical context.",
    Audience.GEOGRAPHY: "Places, maps, regions.",
    Audience.CHEMISTRY: "Reactions and lab safety.",
    Audience.BIOLOGY: "Life systems and careful claims.",
    Audience.PHYSICS: "Models, units, worked problems.",
    Audience.MEDICINE: "Medical literacy only — not a clinician.",
    Audience.NURSING: "Care workflows — not clinical orders.",
    Audience.PHARMACY: "Med education — not prescribing.",
    Audience.DENTAL: "Oral health literacy — not a dentist.",
    Audience.VETERINARY: "Animal health literacy — not a vet.",
    Audience.MILITARY: "Doctrine language carefully; no operational harm.",
    Audience.FIRE: "Prevention and response basics.",
    Audience.POLICE: "Civic process — careful and lawful.",
    Audience.GEOLOGY: "Rocks, earth processes, hazards.",
    Audience.OCEAN: "Marine systems and coasts.",
    Audience.ARCHAEOLOGY: "Sites, methods, careful claims.",
    Audience.LINGUISTICS: "Language science, not just tutoring.",
    Audience.FITNESS: "Gym and training — progressive plans, form cues.",
    Audience.YOGA: "Yoga and mindful movement — alignment and breath.",
    Audience.CLIMBING: "Rock climbing and bouldering — technique and safety.",
    Audience.GOLF: "Golf craft — swing, course management, practice.",
    Audience.FISHING: "Fishing — tackle, technique, local seasons.",
    Audience.SWIMMING: "Swim technique, sets, and open water.",
    Audience.SKIING: "Ski and snowboard — technique and mountain safety.",
    Audience.MOTORCYCLE: "Rides, gear, and maintenance — safety first.",
    Audience.DRONE: "Drones and aerial ops — rules and flight craft.",
    Audience.GAME_DEV: "Game development — systems, loops, engines.",
    Audience.ANIMATION: "Motion and animation craft — timing and arcs.",
    Audience.POETRY: "Poems and verse — form, image, revision.",
    Audience.MAKEUP: "Makeup looks and technique — inclusive tips.",
    Audience.HAIR: "Hair care, cuts, and styling.",
    Audience.SKINCARE: "Skin routines — gentle, evidence-aware (not medical).",
    Audience.WEDDING: "Wedding planning — timeline, budget, vendors.",
    Audience.PREGNANCY: "Pregnancy literacy — not medical care.",
    Audience.SLEEP: "Sleep hygiene and schedules — not a clinic.",
    Audience.FIRST_AID: "First-aid education — emergency basics.",
    Audience.PUBLIC_HEALTH: "Population health, prevention, systems.",
    Audience.ML_AI: "Machine learning and AI systems — careful methods.",
    Audience.SRE: "Site reliability — SLOs, toil, incidents.",
    Audience.SYSTEM_DESIGN: "Distributed systems design interviews & architecture.",
    Audience.TECH_WRITING: "Docs that humans can actually follow.",
    Audience.PROJECT_MGMT: "Project management — plans, risks, delivery.",
    Audience.AGILE: "Scrum/Kanban rituals that stay useful.",
    Audience.REMOTE_WORK: "Distributed teams and home office systems.",
    Audience.CONTENT_CREATOR: "YouTube/TikTok/newsletter craft and systems.",
    Audience.SEO: "Search visibility — technical + content SEO.",
    Audience.BRAND: "Brand strategy, voice, and identity systems.",
    Audience.NEGOTIATION: "Deal craft — prep, BATNA, fair outcomes.",
    Audience.PATENT: "Patents and IP literacy — not legal advice.",
    Audience.HOMESCHOOL: "Home education plans and curricula.",
    Audience.TEST_PREP: "SAT/ACT/GRE and exam strategy.",
    Audience.BARTENDING: "Cocktails and bar craft — drink responsibly.",
    Audience.TEA: "Tea types, brewing, and tasting.",
    Audience.BBQ: "Smoke, fire, and outdoor cooking.",
    Audience.BEEKEEPING: "Hives, seasons, and bee health.",
    Audience.AQUARIUM: "Fish tanks — water chemistry and stock.",
    Audience.BIRDING: "Birdwatching — ID, habitats, ethics.",
    Audience.HORSES: "Horses, riding, and stable care.",
    Audience.SURVIVAL: "Outdoor survival and bushcraft — ethics & safety.",
    Audience.SMART_HOME: "Home automation that actually helps.",
    Audience.AUDIO_HIFI: "Speakers, DACs, and listening rooms.",
    Audience.WATCHES: "Timepieces — movements, care, collecting.",
    Audience.JEWELRY: "Making and choosing jewelry.",
    Audience.CERAMICS: "Clay, wheels, glazes, kilns.",
    Audience.CALLIGRAPHY: "Letterforms, pens, and practice drills.",
    Audience.LEGO: "Builds, MOCs, and brick systems.",
    Audience.HAM_RADIO: "Amateur radio — license, antennas, ops.",
    Audience.QUANT: "Quant trading research — careful, not advice.",
    Audience.FRANCHISE: "Franchise evaluation and ops basics.",
    Audience.RESTAURANT: "Restaurant ops — menu, labor, service.",
    Audience.PROPERTY_MGMT: "Rentals, tenants, and maintenance ops.",
    Audience.PLUMBING: "Pipes and fixtures — when to call a pro.",
    Audience.ELECTRICAL_TRADE: "Home electrical — safety, pro when needed.",
    Audience.HVAC: "Heating, cooling, air quality systems.",
    Audience.MOVING: "Packing, logistics, and settling in.",
    Audience.DECLUTTER: "Organize and let go without shame.",
    Audience.DIGITAL_NOMAD: "Location-independent work and travel systems.",
    Audience.EXPAT: "Living abroad — practical relocation literacy.",
    Audience.NEURODIVERSITY: "ADHD/autism-aware practical support — not clinical.",
    Audience.DISABILITY: "Access and accommodations — person-first practical help.",
    Audience.PHYSICAL_THERAPY: "Rehab movement literacy — not clinical orders.",
    Audience.OPTOMETRY: "Eye health literacy — not an optometrist.",
    Audience.GENETICS: "Genes and inheritance — careful science.",
    Audience.BIOTECH: "Biotech industry and lab concepts carefully.",
    Audience.SPACEFLIGHT: "Rockets, orbits, and mission concepts.",
    Audience.URBAN_PLANNING: "Cities, zoning, transit, public space.",
    Audience.DJ: "DJ craft — mixing, crates, and gigs.",
    Audience.GUITAR: "Guitar technique, songs, and practice.",
    Audience.PIANO: "Piano technique, repertoire, practice.",
    Audience.SINGING: "Voice, breath, and repertoire care.",
    Audience.IMPROV: "Improvisational theater and yes-and craft.",
    Audience.FACILITATION: "Workshops and meetings that go somewhere.",
    Audience.UNION: "Workplace organizing literacy — lawful framing.",
    Audience.CAMPAIGN: "Electoral and issue campaigns — civic craft.",
    Audience.COCKTAILS: "Classic and modern drinks — responsible.",
    Audience.FERMENTATION: "Ferments — safety and process.",
    Audience.FORAGING: "Wild plants — ID carefully, ethics first.",
    Audience.MYCOLOGY: "Fungi science and cultivation carefully.",
    Audience.PERMACULTURE: "Design for resilient food systems.",
    Audience.TINY_HOME: "Small-space living and builds.",
    Audience.HOME_THEATER: "AV rooms, projectors, sound.",
    Audience.STREAMING: "Live streaming setup and growth.",
    Audience.OPEN_SOURCE: "OSS contribution and community norms.",
    Audience.DOCUMENTATION_SITE: "Doc portals, IA, and versioning.",
    Audience.OBSERVABILITY: "Logs, metrics, traces that diagnose reality.",
    Audience.PLATFORM_ENG: "Internal platforms and developer experience.",
    Audience.PRODUCT_MARKETING: "Product marketing — launches and narrative.",
    Audience.COPYWRITING: "Conversion-aware words that still sound human.",
    Audience.AFFILIATE: "Affiliate marketing ethics and systems.",
    Audience.AMAZON_FBA: "Amazon seller ops and inventory.",
    Audience.ETSY: "Handmade/shop listings and craft commerce.",
    Audience.GRANT_WRITING: "Proposals that fund real work.",
    Audience.BOARD_GOVERNANCE: "Boards, fiduciary duty, and agendas.",
    Audience.HIGHER_ED: "Colleges, admissions, academic life.",
    Audience.SPECIAL_ED: "IEPs and inclusive teaching supports.",
    Audience.ESL: "English learning support for speakers of other languages.",
    Audience.MEETING: "Agendas, notes, and decisions that stick.",
    Audience.OKRS: "Objectives and key results that actually work.",
    Audience.CHANGE_MGMT: "Org change that people can absorb.",
    Audience.VC: "Startup investing literacy — not investment advice.",
    Audience.CROWDFUNDING: "Campaigns, rewards, and backers.",
    Audience.DISASTER_PREP: "Household readiness for emergencies.",
    Audience.HUMANITARIAN: "Aid and relief literacy — careful and ethical.",
    Audience.LOCAL_GOV: "Cities, towns, and civic process.",
    Audience.HOUSING: "Housing systems, affordability, tenant rights literacy.",
    Audience.FOOD_SECURITY: "Access to food systems and mutual aid.",
    Audience.ZERO_WASTE: "Waste reduction without purity culture.",
    Audience.COMPOSTING: "Compost systems that actually work.",
    Audience.SOLAR_HOME: "Rooftop solar and storage basics.",
    Audience.EV: "Electric vehicles — charging and ownership.",
    Audience.MOTORSPORTS: "Track, racing, and car performance.",
    Audience.SKATEBOARDING: "Skateboarding tricks and safety.",
    Audience.SURFING: "Waves, boards, and ocean sense.",
    Audience.KAYAKING: "Paddling, safety, and waterways.",
    Audience.ROWING: "Crew, erg, and boat craft.",
    Audience.TRIATHLON: "Swim-bike-run training systems.",
    Audience.POWERLIFTING: "Squat, bench, deadlift programming.",
    Audience.BODYBUILDING: "Hypertrophy, posing, contest prep literacy.",
    Audience.CALISTHENICS: "Bodyweight strength progressions.",
    Audience.PARKOUR: "Movement training — safe progressions only.",
    Audience.TENNIS: "Tennis technique, strategy, and practice.",
    Audience.BASKETBALL: "Hoops — skills, plays, training.",
    Audience.SOCCER: "Football/soccer skills and tactics.",
    Audience.BASEBALL: "Hitting, pitching, fielding craft.",
    Audience.HOCKEY: "Ice hockey skills and systems.",
    Audience.VOLLEYBALL: "Serve, set, spike, systems.",
    Audience.BOXING: "Boxing technique and training — safety first.",
    Audience.WRESTLING: "Wrestling technique and training.",
    Audience.FENCING: "Foil, epee, sabre craft.",
    Audience.ARCHERY: "Bow technique and form.",
    Audience.SAILING: "Boats, wind, and seamanship.",
    Audience.HIKING: "Trails, gear, and mountain sense.",
    Audience.CAMPING: "Campsites, gear, and outdoor nights.",
    Audience.BACKPACKING: "Multi-day trails and ultralight systems.",
    Audience.CROSSFIT: "Functional fitness WODs and scaling.",
    Audience.PILATES: "Core, control, and mat/reformer work.",
    Audience.GYMNASTICS: "Skills, strength, and progressions.",
    Audience.ICE_SKATING: "Edges, freestyle, and rink craft.",
    Audience.OLYMPIC_LIFTING: "Snatch and clean & jerk craft.",
    Audience.DRUMS: "Drum kit technique and grooves.",
    Audience.BASS: "Bass guitar lines and groove.",
    Audience.VIOLIN: "Strings technique and practice.",
    Audience.MUSIC_PRODUCTION: "DAWs, mix, and arrangement.",
    Audience.SOUND_DESIGN: "SFX, synthesis, and sonic texture.",
    Audience.VOICEOVER: "VO scripts, delivery, and home booth.",
    Audience.SCREENWRITING: "Scripts, structure, and scenes.",
    Audience.NOVEL: "Long-form fiction craft and revision.",
    Audience.BLOGGING: "Posts, voice, and publishing cadence.",
    Audience.JOURNALING: "Reflective writing systems.",
    Audience.TRANSLATION: "Cross-language meaning and register.",
    Audience.SIGN_LANGUAGE: "ASL and signed languages — respectful learning.",
    Audience.CROCHET: "Hooks, stitches, and patterns.",
    Audience.EMBROIDERY: "Stitches, hoops, and motifs.",
    Audience.QUILTING: "Blocks, batting, and finishing.",
    Audience.COSPLAY: "Costumes, props, and con prep.",
    Audience.MAGIC_TRICKS: "Sleight of hand and performance.",
    Audience.MODEL_BUILDING: "Scale models, kits, and finishing.",
    Audience.LANDSCAPING: "Yards, hardscape, and outdoor design.",
    Audience.ROOFING: "Roofs and weatherproofing — when to call a pro.",
    Audience.PAINTING_TRADE: "Prep, paint, and finish work.",
    Audience.FLOORING: "Floors — install and care basics.",
    Audience.CARPENTRY: "Framing, trim, and site carpentry.",
    Audience.APPLIANCE_REPAIR: "Home appliances — diagnose carefully.",
    Audience.PEST_CONTROL: "Home pests — safe, practical steps.",
    Audience.AUTO_BODY: "Bodywork, paint, and dent craft.",
    Audience.DOG_TRAINING: "Training and behavior — force-free first.",
    Audience.CAT_CARE: "Cats — health, behavior, enrichment.",
    Audience.CHICKENS: "Backyard flocks and coops.",
    Audience.REPTILES: "Herps — husbandry and enclosure care.",
    Audience.CAREGIVING: "Supporting someone with practical care systems.",
    Audience.CHRONIC_ILLNESS: "Living with long-term conditions — not medical care.",
    Audience.MASSAGE: "Bodywork literacy — not clinical therapy.",
    Audience.MENTAL_FITNESS: "Stress skills and habits — not therapy.",
    Audience.FERTILITY: "Fertility literacy — not medical advice.",
    Audience.LACTATION: "Feeding support literacy — not clinical care.",
    Audience.PSYCHOLOGY: "Psych science carefully — not therapy.",
    Audience.NEUROSCIENCE: "Brain science carefully explained.",
    Audience.ECONOMICS: "Incentives, markets, and models carefully.",
    Audience.SOCIOLOGY: "Social structures and research literacy.",
    Audience.ANTHROPOLOGY: "Culture, fieldwork, careful comparison.",
    Audience.MATERIALS_SCIENCE: "Materials, properties, and selection.",
    Audience.ECOLOGY: "Ecosystems and conservation science.",
    Audience.PERSONAL_FINANCE: "Budgets and money systems — not advice.",
    Audience.RETIREMENT: "Retirement planning literacy — not advice.",
    Audience.ESTATE_PLANNING: "Wills and estates literacy — not legal advice.",
    Audience.SIDE_HUSTLE: "Extra income projects with realistic math.",
    Audience.REAL_ESTATE_INVESTING: "Property investing literacy — not advice.",
    Audience.IMPORT_EXPORT: "Cross-border trade logistics literacy.",
    Audience.INVENTORY: "Stock, SKUs, and replenishment.",
    Audience.DATA_ENGINEERING: "Pipelines, warehouses, and quality.",
    Audience.SPREADSHEETS: "Excel/Sheets formulas and models.",
    Audience.NOCODE: "No/low-code builders and automations.",
    Audience.WORDPRESS: "WP sites, themes, and plugins.",
    Audience.PRIVACY: "Privacy hygiene and data minimization.",
    Audience.PROMPT_ENG: "Prompting LLMs effectively and safely.",
    Audience.KUBERNETES: "K8s workloads, networking, ops.",
    Audience.GRAPHICS_PROG: "Shaders, GPU, and real-time graphics.",
    Audience.COMPILER: "Languages, IR, and compilation pipelines.",
    Audience.API_DESIGN: "HTTP APIs that stay pleasant.",
    Audience.FRONTEND: "UI implementation in the browser.",
    Audience.BACKEND: "Servers, services, and data paths.",
    Audience.PARENTING_TEENS: "Raising teens with respect and boundaries.",
    Audience.ADOPTION: "Adoption process literacy — careful and respectful.",
    Audience.DIVORCE: "Separation logistics literacy — not legal advice.",
    Audience.GRIEF: "Bereavement support language — not therapy.",
    Audience.MINIMALISM: "Enough-ness without purity culture.",
    Audience.LUXURY: "High-end goods literacy without snobbery.",
    Audience.THRIFTING: "Secondhand finds and upcycling.",
    Audience.ROAD_TRIP: "Routes, packing, and drive days.",
    Audience.CRUISE: "Cruise planning and ship life.",
    Audience.FOOD_TRAVEL: "Eating well on the road.",
    Audience.TUTORING: "1:1 teaching that builds independence.",
    Audience.CURRICULUM: "Scope, sequence, and learning design.",
    Audience.EARLY_CHILDHOOD: "Ages 0–8 learning and care.",
    Audience.MONTESSORI: "Montessori materials and prepared environment.",
    Audience.EDTECH: "Learning products and classroom tech.",
    Audience.NEIGHBORHOOD: "Local community and block-level action.",
    Audience.VOLUNTEERING: "Giving time that actually helps.",
    Audience.FUNDRAISING_EVENTS: "Galas, drives, and donor nights.",
    Audience.PHOTOGRAPHY_EDITING: "Lightroom/Photoshop craft.",
    Audience.VIDEO_EDITING: "Cuts, sound, and delivery formats.",
    Audience.PODCAST_EDITING: "Audio cleanup and episode assembly.",
    Audience.NEWSLETTER: "Email issues people open.",
    Audience.COMMUNITY_MGMT: "Healthy online communities and mods.",
    Audience.CUSTOMER_SUCCESS: "Retention, onboarding, and QBRs.",
    Audience.REVENUE_OPS: "Funnel systems across sales/marketing.",
    Audience.PEOPLE_OPS: "People systems beyond classic HR forms.",
    Audience.OFFICE_ADMIN: "Calendars, travel, and ops glue work.",
    Audience.RESEARCH_METHODS: "Study design and careful inference.",
    Audience.STATISTICS_APPLIED: "Applied stats for real datasets.",
    Audience.CLIMATE_ACTION: "Personal and civic climate steps.",
    Audience.RECYCLING: "What actually gets recycled where you live.",
    Audience.WATER_CONSERVATION: "Use less water without misery.",
    Audience.HOME_SECURITY: "Locks, cameras, and sensible hardening.",
    Audience.CYBER_HYGIENE: "Everyday account and device safety.",
    Audience.PASSWORD_SECURITY: "Passwords and passkeys done right.",
    Audience.BROWSER_EXT: "Extensions that help without spying.",
    Audience.EMAIL_PRODUCTIVITY: "Inbox systems that stick.",
    Audience.NOTE_TAKING: "Capture systems that you can find later.",
    Audience.SPEED_READING: "Faster reading without losing meaning.",
    Audience.DEBATE: "Argument structure and fair rebuttal.",
    Audience.PUBLIC_POLICY_ANALYSIS: "Options memos with tradeoffs.",
    Audience.MAPS_GIS: "Maps, layers, and spatial data.",
    Audience.CARTOGRAPHY: "Map design people can read.",
    Audience.ASTROPHOTOGRAPHY: "Night sky capture and stacking.",
    Audience.METEOROLOGY_HOBBY: "Hobby forecasting and storm sense.",
    Audience.AMATEUR_ASTRONOMY: "Scopes, skies, and observing logs.",
    Audience.BOARD_GAMES: "Modern board game design and play.",
    Audience.PUZZLES: "Crosswords, logic, and puzzle craft.",
    Audience.RUBIKS: "Speedcubing methods and practice.",
    Audience.ORIGAMI: "Paper folding diagrams and design.",
    Audience.KNIFE_SKILLS: "Kitchen knife technique and safety.",
    Audience.MEAL_PREP: "Batch cooking systems that last.",
    Audience.KETO: "Keto pattern literacy — not medical diet therapy.",
    Audience.VEGAN_COOKING: "Plant-based cooking that tastes good.",
    Audience.GLUTEN_FREE: "GF cooking and swaps carefully.",
    Audience.SOUS_VIDE: "Precision water-bath cooking.",
    Audience.SMOKING_MEAT: "Low-and-slow smoke craft.",
    Audience.PICKLEBALL: "Dinks, drives, and kitchen craft.",
    Audience.BADMINTON: "Smash, clear, and footwork.",
    Audience.TABLE_TENNIS: "Spin, serve, and footwork.",
    Audience.RUGBY: "Scrums, lines, and contact skill.",
    Audience.CRICKET: "Batting, bowling, fielding craft.",
    Audience.SOFTBALL: "Pitching, hitting, defensive play.",
    Audience.LACROSSE: "Stick skills and team systems.",
    Audience.WATER_POLO: "Swimming strength and ball skills.",
    Audience.DIVING_SPORT: "Dives, entries, and board work.",
    Audience.SYNCHRONIZED_SWIM: "Routines, figures, and team timing.",
    Audience.EQUESTRIAN_SPORT: "Dressage, jumping, eventing.",
    Audience.ESPORTS: "Competitive gaming craft and team play.",
    Audience.SPEEDRUNNING: "Routes, splits, and optimization.",
    Audience.YOGA_THERAPY: "Therapeutic yoga literacy — not clinical care.",
    Audience.MOBILITY: "Joints, ranges, and soft tissue care.",
    Audience.BREATHWORK: "Breath practices carefully and safely.",
    Audience.SPANISH: "Spanish learning and usage.",
    Audience.FRENCH: "French learning and usage.",
    Audience.GERMAN: "German learning and usage.",
    Audience.JAPANESE: "Japanese learning — kana, kanji, usage.",
    Audience.MANDARIN: "Mandarin Chinese learning and tones.",
    Audience.KOREAN: "Korean learning — hangul and usage.",
    Audience.ITALIAN: "Italian learning and usage.",
    Audience.PORTUGUESE: "Portuguese learning and usage.",
    Audience.ARABIC: "Arabic learning — script and dialects carefully.",
    Audience.HINDI: "Hindi learning and Devanagari.",
    Audience.GREEK_LANG: "Modern Greek learning and usage.",
    Audience.LATIN: "Classical Latin reading and grammar.",
    Audience.UKULELE: "Uke chords, songs, and strumming.",
    Audience.SAXOPHONE: "Sax tone, reeds, and repertoire.",
    Audience.TRUMPET: "Brass embouchure and range.",
    Audience.FLUTE: "Tone, breath, and articulation.",
    Audience.HARMONICA: "Bends, positions, and blues craft.",
    Audience.BANJO: "Rolls, clawhammer, and bluegrass.",
    Audience.MUSIC_THEORY: "Harmony, form, and ear training.",
    Audience.GRAPHIC_DESIGN: "Layout, type, and visual systems.",
    Audience.ILLUSTRATION: "Drawing craft and visual storytelling.",
    Audience.UX_WRITING: "Microcopy that guides without friction.",
    Audience.STORYBOARD: "Shots on paper before the camera.",
    Audience.COLOR_GRADING: "Looks, scopes, and finishing.",
    Audience.LIGHTING_DESIGN: "Light for stage, film, or space.",
    Audience.COSTUME_DESIGN: "Wardrobe for stage and screen.",
    Audience.SET_DESIGN: "Spaces that tell the story.",
    Audience.PASTRY: "Laminated doughs and pastry craft.",
    Audience.BREAD: "Loaves, fermentation, and crumb.",
    Audience.CHOCOLATE: "Tempering, truffles, and cacao craft.",
    Audience.CHEESE: "Cheesemaking and pairing literacy.",
    Audience.CHARCUTERIE: "Cured meats carefully and safely.",
    Audience.PRESERVING: "Canning, pickling, jams — safety first.",
    Audience.INDIAN_COOKING: "Spices, dals, and regional techniques.",
    Audience.CHINESE_COOKING: "Wok craft and regional cuisines.",
    Audience.MEXICAN_COOKING: "Salsas, masa, and regional dishes.",
    Audience.ITALIAN_COOKING: "Pasta, risotto, regional Italian food.",
    Audience.JAPANESE_COOKING: "Washoku techniques and home Japanese food.",
    Audience.BBQ_SAUCES: "Sauces, rubs, and regional BBQ styles.",
    Audience.COFFEE_ROASTING: "Roast curves and bean development.",
    Audience.LATTE_ART: "Milk texture and patterns.",
    Audience.HOUSEPLANTS: "Indoor plant care that sticks.",
    Audience.HYDROPONICS: "Soilless growing systems.",
    Audience.BONSAI: "Styling, wiring, and tree health.",
    Audience.AQUAPONICS: "Fish + plants systems.",
    Audience.LAWN_CARE: "Grass, soil, and seasonal care.",
    Audience.IRRIGATION: "Water systems for yards and farms.",
    Audience.POOL_CARE: "Chemistry and maintenance.",
    Audience.FIREPLACE: "Fires, chimneys, and stove safety.",
    Audience.DENTAL_HYGIENE: "Oral care literacy — not a dentist.",
    Audience.PHARMACOLOGY: "Drug mechanisms literacy — not prescribing.",
    Audience.RADIOLOGY_LITERACY: "Imaging literacy — not a radiologist.",
    Audience.NUTRITION_SCIENCE: "Evidence-aware food science — not medical diets.",
    Audience.EPIDEMIOLOGY: "Population disease patterns carefully.",
    Audience.BIOSTATISTICS: "Stats for health studies carefully.",
    Audience.BOOKKEEPING: "Day-to-day books and reconciliations.",
    Audience.PAYROLL: "Pay runs and withholdings literacy.",
    Audience.BILLING: "Invoices, AR, and collections hygiene.",
    Audience.PRICING: "Price strategy and packaging.",
    Audience.SALES_ENABLEMENT: "Collateral and coaching that sells.",
    Audience.PARTNERSHIPS: "Alliances, channels, and co-selling.",
    Audience.CUSTOMER_RESEARCH: "Jobs-to-be-done and discovery.",
    Audience.ANALYTICS: "Product/marketing measurement systems.",
    Audience.AB_TESTING: "Experiments with real statistical care.",
    Audience.RUST_LANG: "Ownership, lifetimes, and safe systems.",
    Audience.GO_LANG: "Simple concurrent services in Go.",
    Audience.PYTHON_DATA: "pandas, numpy, analysis notebooks.",
    Audience.SQL_ANALYTICS: "Analytical SQL that answers questions.",
    Audience.TERRAFORM: "Infra as code with plan/apply discipline.",
    Audience.ANSIBLE: "Config management and playbooks.",
    Audience.CICD: "Pipelines that ship safely.",
    Audience.DOCKER: "Images, containers, and local stacks.",
    Audience.LINUX_ADMIN: "Servers, users, and sysadmin craft.",
    Audience.NETWORK_SECURITY: "Defensive network hardening.",
    Audience.PENTEST_DEFENSE: "Defensive security testing literacy — ethical only.",
    Audience.THREAT_MODEL: "STRIDE-style risk thinking.",
    Audience.INCIDENT_RESPONSE: "Detect, contain, learn.",
    Audience.QA_TESTING: "Test strategy, cases, and quality signals.",
    Audience.MOBILE_QA: "Device matrices and mobile test craft.",
    Audience.ACCESSIBILITY_ENG: "Engineering accessible products.",
    Audience.PERFORMANCE_WEB: "Core Web Vitals and fast UX.",
    Audience.SEO_TECHNICAL: "Crawl, index, structured data.",
    Audience.TAX_PREP: "Filing literacy — not a preparer.",
    Audience.INSURANCE_CLAIMS: "Claims process literacy — not a broker.",
    Audience.CAR_BUYING: "Research, negotiate, and own wisely.",
    Audience.HOME_BUYING: "Offers, inspections, closing literacy.",
    Audience.RENTING: "Leases, roommates, and tenant practicalities.",
    Audience.COLLEGE_APPS: "Essays, lists, and application systems.",
    Audience.SCHOLARSHIPS: "Finding and applying for aid.",
    Audience.STUDY_ABROAD: "Programs, packing, and cultural prep.",
    Audience.INTERNSHIP: "Finding, applying, and succeeding as an intern.",
    Audience.CAREER_CHANGE: "Pivots with honest transferable stories.",
    Audience.LINKEDIN: "Messages, profile, and outreach — human, not spam.",
    Audience.WHATSAPP: "Chats and replies — natural, paste-ready, you send.",
    Audience.NETWORKING_CAREER: "Warm intros and relationship craft.",
    Audience.HOA_LIVING: "HOA rules and neighborly process.",
    Audience.COOP_HOUSING: "Co-ops, shares, and shared living.",
    Audience.COMMUNITY_GARDEN: "Shared plots and garden groups.",
    Audience.MUTUAL_AID: "Neighbor help systems that last.",
    Audience.FISHKEEPING: "Aquarium husbandry beyond setup.",
    Audience.TERRARIUM: "Closed ecosystems and vivariums.",
    Audience.ANTKEEPING: "Formicariums and colony care.",
    Audience.BEEKEEPING_ADVANCED: "Splits, queens, and seasonal management.",
    Audience.FOUNTAIN_PEN: "Nibs, inks, and paper.",
    Audience.STATIONERY: "Notebooks, pens, and paper systems.",
    Audience.MECHANICAL_KEYBOARD: "Switches, kits, and typing feel.",
    Audience.PC_BUILDING: "Parts, thermals, and cable sense.",
    Audience.HOME_LAB: "Self-hosted servers and learning labs.",
    Audience.THREE_D_MODELING: "Meshes, CAD, and printable forms.",
    Audience.CNC: "Toolpaths, feeds, and machine safety.",
    Audience.LASER_CUTTING: "Vectors, materials, and safe operation.",
    Audience.RESIN_PRINTING: "SLA/DLP prints and safety.",
    Audience.FILAMENT_PRINTING: "FDM settings and print quality.",
    Audience.MEDITATION: "Practice structures, not dogma.",
    Audience.STOICISM: "Practical Stoic exercises carefully.",
    Audience.JOURNAL_PROMPTS: "Prompts that open reflection.",
    Audience.HABIT_BUILDING: "Tiny systems that compound.",
    Audience.TIME_BLOCKING: "Calendar systems that protect focus.",
    Audience.SECOND_BRAIN: "Capture-to-action knowledge systems.",
    Audience.PACKING: "Light, complete packing systems.",
    Audience.TRAVEL_PHOTOGRAPHY: "Shooting stories on the move.",
    Audience.SOLO_TRAVEL: "Independent trips with safety sense.",
    Audience.FAMILY_TRAVEL: "Trips with kids that stay sane.",
    Audience.BUDGET_TRAVEL: "More trip for less money.",
    Audience.POINTS_MILES: "Loyalty programs carefully.",
    Audience.REAL_ESTATE_PHOTO: "Listing photos that sell space.",
    Audience.STAGING: "Rooms that photograph and show well.",
    Audience.INTERIOR_STYLING: "Vignettes and finish layers.",
    Audience.EVENT_PHOTOGRAPHY: "Weddings, parties, candid craft.",
    Audience.PORTRAIT_PHOTO: "People, light, and direction.",
    Audience.STREET_PHOTO: "Public scenes with ethics.",
    Audience.WILDLIFE_PHOTO: "Animals, ethics, long glass.",
    Audience.ASTRO_IMAGING_PROC: "Stack, stretch, and finish night data.",
    Audience.PLAIN: "Plain language only — no terminal steps or code jargon.",
}

_HEADER_TITLES: dict[Audience, str] = {
    Audience.CODE: "Ultracode",
    Audience.ACADEMIC: "Academic help",
    Audience.STUDENT: "Student help",
    Audience.WRITER: "Writer help",
    Audience.BUSINESS: "Business help",
    Audience.DESIGN: "Design help",
    Audience.JOB: "Career help",
    Audience.TEACHER: "Teacher help",
    Audience.DATA: "Data help",
    Audience.FOUNDER: "Founder help",
    Audience.LEGAL: "Document help",
    Audience.PARENT: "Family help",
    Audience.MARKETING: "Marketing help",
    Audience.SALES: "Sales help",
    Audience.FINANCE: "Finance help",
    Audience.PRODUCT: "Product help",
    Audience.SUPPORT: "Support help",
    Audience.SCIENCE: "Science help",
    Audience.LANGUAGE: "Language help",
    Audience.CREATIVE: "Creative help",
    Audience.HEALTH: "Wellness help",
    Audience.NONPROFIT: "Nonprofit help",
    Audience.POLICY: "Policy help",
    Audience.REAL_ESTATE: "Real estate help",
    Audience.TRAVEL: "Travel help",
    Audience.COOKING: "Cooking help",
    Audience.GAMING: "Gaming help",
    Audience.SPORTS: "Sports help",
    Audience.HR: "HR help",
    Audience.JOURNALISM: "Journalism help",
    Audience.ACCESSIBILITY: "Accessibility help",
    Audience.ENGINEERING: "Engineering help",
    Audience.SECURITY: "Security help",
    Audience.HOSPITALITY: "Hospitality help",
    Audience.EVENTS: "Events help",
    Audience.FASHION: "Fashion help",
    Audience.DIY: "DIY help",
    Audience.ENVIRONMENT: "Environment help",
    Audience.SPIRITUAL: "Spiritual help",
    Audience.SENIOR: "Senior help",
    Audience.AUTOMOTIVE: "Auto help",
    Audience.AGRICULTURE: "Agriculture help",
    Audience.MUSIC: "Music help",
    Audience.PHOTOGRAPHY: "Photography help",
    Audience.FILM: "Film help",
    Audience.PODCAST: "Podcast help",
    Audience.ARCHITECTURE: "Architecture help",
    Audience.INTERIOR: "Interior help",
    Audience.INSURANCE: "Insurance help",
    Audience.TAX: "Tax help",
    Audience.INVESTING: "Investing help",
    Audience.CRYPTO: "Crypto help",
    Audience.RETAIL: "Retail help",
    Audience.ECOMMERCE: "Ecommerce help",
    Audience.LOGISTICS: "Logistics help",
    Audience.MANUFACTURING: "Manufacturing help",
    Audience.CONSTRUCTION: "Construction help",
    Audience.ROBOTICS: "Robotics help",
    Audience.MATH: "Math help",
    Audience.PHILOSOPHY: "Philosophy help",
    Audience.PETS: "Pets help",
    Audience.CHILDCARE: "Childcare help",
    Audience.IMMIGRATION: "Immigration help",
    Audience.THERAPY: "Emotional support",
    Audience.LIBRARY: "Library help",
    Audience.THEATER: "Theater help",
    Audience.DANCE: "Dance help",
    Audience.WEATHER: "Weather help",
    Audience.ASTRONOMY: "Astronomy help",
    Audience.COMPLIANCE: "Compliance help",
    Audience.OPERATIONS: "Ops help",
    Audience.PROCUREMENT: "Procurement help",
    Audience.QUALITY: "QA help",
    Audience.GROWTH: "Growth help",
    Audience.UX_RESEARCH: "UX research help",
    Audience.STATS: "Stats help",
    Audience.GENEALOGY: "Genealogy help",
    Audience.COLLECTING: "Collecting help",
    Audience.OUTDOORS: "Outdoors help",
    Audience.GARDENING: "Gardening help",
    Audience.BAKING: "Baking help",
    Audience.COFFEE: "Coffee help",
    Audience.WINE: "Wine help",
    Audience.BEER: "Beer help",
    Audience.AVIATION: "Aviation help",
    Audience.MARITIME: "Maritime help",
    Audience.ENERGY: "Energy help",
    Audience.TELECOM: "Telecom help",
    Audience.MEDIA: "Media help",
    Audience.PR: "PR help",
    Audience.SOCIAL_WORK: "Social work help",
    Audience.ACCOUNTING: "Accounting help",
    Audience.ACTING: "Acting help",
    Audience.COMEDY: "Comedy help",
    Audience.WOODWORKING: "Woodworking help",
    Audience.METALWORKING: "Metalwork help",
    Audience.ELECTRONICS: "Electronics help",
    Audience.PRINTING_3D: "3D printing help",
    Audience.SEWING: "Sewing help",
    Audience.KNITTING: "Knitting help",
    Audience.CHESS: "Chess help",
    Audience.TABLETOP: "Tabletop help",
    Audience.ANIME: "Anime help",
    Audience.COMICS: "Comics help",
    Audience.SCUBA: "Scuba help",
    Audience.CYCLING: "Cycling help",
    Audience.RUNNING: "Running help",
    Audience.MARTIAL_ARTS: "Martial arts help",
    Audience.NUTRITION: "Nutrition help",
    Audience.PRODUCTIVITY: "Productivity help",
    Audience.PKM: "PKM help",
    Audience.DEVOPS: "DevOps help",
    Audience.CLOUD: "Cloud help",
    Audience.NETWORKING: "Networking help",
    Audience.DATABASE: "Database help",
    Audience.MOBILE: "Mobile help",
    Audience.WEBDEV: "Web help",
    Audience.EMBEDDED: "Embedded help",
    Audience.IOT: "IoT help",
    Audience.ARVR: "AR/VR help",
    Audience.FREELANCE: "Freelance help",
    Audience.CONSULTING: "Consulting help",
    Audience.COACHING: "Coaching help",
    Audience.SPEAKING: "Speaking help",
    Audience.RELATIONSHIPS: "Relationships help",
    Audience.DATING: "Dating help",
    Audience.HISTORY: "History help",
    Audience.GEOGRAPHY: "Geography help",
    Audience.CHEMISTRY: "Chemistry help",
    Audience.BIOLOGY: "Biology help",
    Audience.PHYSICS: "Physics help",
    Audience.MEDICINE: "Medicine help",
    Audience.NURSING: "Nursing help",
    Audience.PHARMACY: "Pharmacy help",
    Audience.DENTAL: "Dental help",
    Audience.VETERINARY: "Vet help",
    Audience.MILITARY: "Military help",
    Audience.FIRE: "Fire safety help",
    Audience.POLICE: "Public safety help",
    Audience.GEOLOGY: "Geology help",
    Audience.OCEAN: "Ocean help",
    Audience.ARCHAEOLOGY: "Archaeology help",
    Audience.LINGUISTICS: "Linguistics help",
    Audience.FITNESS: "Fitness help",
    Audience.YOGA: "Yoga help",
    Audience.CLIMBING: "Climbing help",
    Audience.GOLF: "Golf help",
    Audience.FISHING: "Fishing help",
    Audience.SWIMMING: "Swimming help",
    Audience.SKIING: "Ski help",
    Audience.MOTORCYCLE: "Motorcycle help",
    Audience.DRONE: "Drone help",
    Audience.GAME_DEV: "Game dev help",
    Audience.ANIMATION: "Animation help",
    Audience.POETRY: "Poetry help",
    Audience.MAKEUP: "Makeup help",
    Audience.HAIR: "Hair help",
    Audience.SKINCARE: "Skincare help",
    Audience.WEDDING: "Wedding help",
    Audience.PREGNANCY: "Pregnancy help",
    Audience.SLEEP: "Sleep help",
    Audience.FIRST_AID: "First aid help",
    Audience.PUBLIC_HEALTH: "Public health help",
    Audience.ML_AI: "ML/AI help",
    Audience.SRE: "SRE help",
    Audience.SYSTEM_DESIGN: "System design help",
    Audience.TECH_WRITING: "Tech writing help",
    Audience.PROJECT_MGMT: "Project help",
    Audience.AGILE: "Agile help",
    Audience.REMOTE_WORK: "Remote work help",
    Audience.CONTENT_CREATOR: "Creator help",
    Audience.SEO: "SEO help",
    Audience.BRAND: "Brand help",
    Audience.NEGOTIATION: "Negotiation help",
    Audience.PATENT: "IP help",
    Audience.HOMESCHOOL: "Homeschool help",
    Audience.TEST_PREP: "Test prep help",
    Audience.BARTENDING: "Bartending help",
    Audience.TEA: "Tea help",
    Audience.BBQ: "BBQ help",
    Audience.BEEKEEPING: "Beekeeping help",
    Audience.AQUARIUM: "Aquarium help",
    Audience.BIRDING: "Birding help",
    Audience.HORSES: "Equestrian help",
    Audience.SURVIVAL: "Survival help",
    Audience.SMART_HOME: "Smart home help",
    Audience.AUDIO_HIFI: "Hi-fi help",
    Audience.WATCHES: "Watches help",
    Audience.JEWELRY: "Jewelry help",
    Audience.CERAMICS: "Ceramics help",
    Audience.CALLIGRAPHY: "Calligraphy help",
    Audience.LEGO: "LEGO help",
    Audience.HAM_RADIO: "Ham radio help",
    Audience.QUANT: "Quant help",
    Audience.FRANCHISE: "Franchise help",
    Audience.RESTAURANT: "Restaurant help",
    Audience.PROPERTY_MGMT: "Property help",
    Audience.PLUMBING: "Plumbing help",
    Audience.ELECTRICAL_TRADE: "Electrical help",
    Audience.HVAC: "HVAC help",
    Audience.MOVING: "Moving help",
    Audience.DECLUTTER: "Declutter help",
    Audience.DIGITAL_NOMAD: "Nomad help",
    Audience.EXPAT: "Expat help",
    Audience.NEURODIVERSITY: "Neurodiversity help",
    Audience.DISABILITY: "Disability help",
    Audience.PHYSICAL_THERAPY: "PT help",
    Audience.OPTOMETRY: "Vision help",
    Audience.GENETICS: "Genetics help",
    Audience.BIOTECH: "Biotech help",
    Audience.SPACEFLIGHT: "Spaceflight help",
    Audience.URBAN_PLANNING: "Urban planning help",
    Audience.DJ: "DJ help",
    Audience.GUITAR: "Guitar help",
    Audience.PIANO: "Piano help",
    Audience.SINGING: "Singing help",
    Audience.IMPROV: "Improv help",
    Audience.FACILITATION: "Facilitation help",
    Audience.UNION: "Labor help",
    Audience.CAMPAIGN: "Campaign help",
    Audience.COCKTAILS: "Mixology help",
    Audience.FERMENTATION: "Fermentation help",
    Audience.FORAGING: "Foraging help",
    Audience.MYCOLOGY: "Mycology help",
    Audience.PERMACULTURE: "Permaculture help",
    Audience.TINY_HOME: "Tiny home help",
    Audience.HOME_THEATER: "Home theater help",
    Audience.STREAMING: "Streaming help",
    Audience.OPEN_SOURCE: "Open source help",
    Audience.DOCUMENTATION_SITE: "Docs site help",
    Audience.OBSERVABILITY: "Observability help",
    Audience.PLATFORM_ENG: "Platform help",
    Audience.PRODUCT_MARKETING: "PMM help",
    Audience.COPYWRITING: "Copywriting help",
    Audience.AFFILIATE: "Affiliate help",
    Audience.AMAZON_FBA: "Amazon FBA help",
    Audience.ETSY: "Etsy help",
    Audience.GRANT_WRITING: "Grant writing help",
    Audience.BOARD_GOVERNANCE: "Board help",
    Audience.HIGHER_ED: "Higher ed help",
    Audience.SPECIAL_ED: "Special ed help",
    Audience.ESL: "ESL help",
    Audience.MEETING: "Meeting help",
    Audience.OKRS: "OKR help",
    Audience.CHANGE_MGMT: "Change help",
    Audience.VC: "VC help",
    Audience.CROWDFUNDING: "Crowdfunding help",
    Audience.DISASTER_PREP: "Disaster prep help",
    Audience.HUMANITARIAN: "Humanitarian help",
    Audience.LOCAL_GOV: "Local gov help",
    Audience.HOUSING: "Housing help",
    Audience.FOOD_SECURITY: "Food security help",
    Audience.ZERO_WASTE: "Zero waste help",
    Audience.COMPOSTING: "Composting help",
    Audience.SOLAR_HOME: "Home solar help",
    Audience.EV: "EV help",
    Audience.MOTORSPORTS: "Motorsports help",
    Audience.SKATEBOARDING: "Skate help",
    Audience.SURFING: "Surfing help",
    Audience.KAYAKING: "Kayak help",
    Audience.ROWING: "Rowing help",
    Audience.TRIATHLON: "Triathlon help",
    Audience.POWERLIFTING: "Powerlifting help",
    Audience.BODYBUILDING: "Bodybuilding help",
    Audience.CALISTHENICS: "Calisthenics help",
    Audience.PARKOUR: "Parkour help",
    Audience.TENNIS: "Tennis help",
    Audience.BASKETBALL: "Basketball help",
    Audience.SOCCER: "Soccer help",
    Audience.BASEBALL: "Baseball help",
    Audience.HOCKEY: "Hockey help",
    Audience.VOLLEYBALL: "Volleyball help",
    Audience.BOXING: "Boxing help",
    Audience.WRESTLING: "Wrestling help",
    Audience.FENCING: "Fencing help",
    Audience.ARCHERY: "Archery help",
    Audience.SAILING: "Sailing help",
    Audience.HIKING: "Hiking help",
    Audience.CAMPING: "Camping help",
    Audience.BACKPACKING: "Backpacking help",
    Audience.CROSSFIT: "CrossFit help",
    Audience.PILATES: "Pilates help",
    Audience.GYMNASTICS: "Gymnastics help",
    Audience.ICE_SKATING: "Ice skating help",
    Audience.OLYMPIC_LIFTING: "Olympic lifting help",
    Audience.DRUMS: "Drums help",
    Audience.BASS: "Bass help",
    Audience.VIOLIN: "Violin help",
    Audience.MUSIC_PRODUCTION: "Music production help",
    Audience.SOUND_DESIGN: "Sound design help",
    Audience.VOICEOVER: "Voiceover help",
    Audience.SCREENWRITING: "Screenwriting help",
    Audience.NOVEL: "Novel help",
    Audience.BLOGGING: "Blogging help",
    Audience.JOURNALING: "Journaling help",
    Audience.TRANSLATION: "Translation help",
    Audience.SIGN_LANGUAGE: "Sign language help",
    Audience.CROCHET: "Crochet help",
    Audience.EMBROIDERY: "Embroidery help",
    Audience.QUILTING: "Quilting help",
    Audience.COSPLAY: "Cosplay help",
    Audience.MAGIC_TRICKS: "Magic help",
    Audience.MODEL_BUILDING: "Model help",
    Audience.LANDSCAPING: "Landscaping help",
    Audience.ROOFING: "Roofing help",
    Audience.PAINTING_TRADE: "House painting help",
    Audience.FLOORING: "Flooring help",
    Audience.CARPENTRY: "Carpentry help",
    Audience.APPLIANCE_REPAIR: "Appliance help",
    Audience.PEST_CONTROL: "Pest control help",
    Audience.AUTO_BODY: "Auto body help",
    Audience.DOG_TRAINING: "Dog training help",
    Audience.CAT_CARE: "Cat care help",
    Audience.CHICKENS: "Chickens help",
    Audience.REPTILES: "Reptiles help",
    Audience.CAREGIVING: "Caregiving help",
    Audience.CHRONIC_ILLNESS: "Chronic illness help",
    Audience.MASSAGE: "Massage help",
    Audience.MENTAL_FITNESS: "Mental fitness help",
    Audience.FERTILITY: "Fertility help",
    Audience.LACTATION: "Lactation help",
    Audience.PSYCHOLOGY: "Psychology help",
    Audience.NEUROSCIENCE: "Neuroscience help",
    Audience.ECONOMICS: "Economics help",
    Audience.SOCIOLOGY: "Sociology help",
    Audience.ANTHROPOLOGY: "Anthropology help",
    Audience.MATERIALS_SCIENCE: "Materials help",
    Audience.ECOLOGY: "Ecology help",
    Audience.PERSONAL_FINANCE: "Personal finance help",
    Audience.RETIREMENT: "Retirement help",
    Audience.ESTATE_PLANNING: "Estate help",
    Audience.SIDE_HUSTLE: "Side hustle help",
    Audience.REAL_ESTATE_INVESTING: "RE investing help",
    Audience.IMPORT_EXPORT: "Trade help",
    Audience.INVENTORY: "Inventory help",
    Audience.DATA_ENGINEERING: "Data eng help",
    Audience.SPREADSHEETS: "Spreadsheet help",
    Audience.NOCODE: "No-code help",
    Audience.WORDPRESS: "WordPress help",
    Audience.PRIVACY: "Privacy help",
    Audience.PROMPT_ENG: "Prompt eng help",
    Audience.KUBERNETES: "Kubernetes help",
    Audience.GRAPHICS_PROG: "Graphics help",
    Audience.COMPILER: "Compiler help",
    Audience.API_DESIGN: "API design help",
    Audience.FRONTEND: "Frontend help",
    Audience.BACKEND: "Backend help",
    Audience.PARENTING_TEENS: "Teen parenting help",
    Audience.ADOPTION: "Adoption help",
    Audience.DIVORCE: "Divorce help",
    Audience.GRIEF: "Grief help",
    Audience.MINIMALISM: "Minimalism help",
    Audience.LUXURY: "Luxury help",
    Audience.THRIFTING: "Thrifting help",
    Audience.ROAD_TRIP: "Road trip help",
    Audience.CRUISE: "Cruise help",
    Audience.FOOD_TRAVEL: "Food travel help",
    Audience.TUTORING: "Tutoring help",
    Audience.CURRICULUM: "Curriculum help",
    Audience.EARLY_CHILDHOOD: "Early childhood help",
    Audience.MONTESSORI: "Montessori help",
    Audience.EDTECH: "EdTech help",
    Audience.NEIGHBORHOOD: "Neighborhood help",
    Audience.VOLUNTEERING: "Volunteering help",
    Audience.FUNDRAISING_EVENTS: "Fundraising events help",
    Audience.PHOTOGRAPHY_EDITING: "Photo editing help",
    Audience.VIDEO_EDITING: "Video editing help",
    Audience.PODCAST_EDITING: "Podcast editing help",
    Audience.NEWSLETTER: "Newsletter help",
    Audience.COMMUNITY_MGMT: "Community help",
    Audience.CUSTOMER_SUCCESS: "Customer success help",
    Audience.REVENUE_OPS: "RevOps help",
    Audience.PEOPLE_OPS: "People ops help",
    Audience.OFFICE_ADMIN: "Office admin help",
    Audience.RESEARCH_METHODS: "Research methods help",
    Audience.STATISTICS_APPLIED: "Applied stats help",
    Audience.CLIMATE_ACTION: "Climate action help",
    Audience.RECYCLING: "Recycling help",
    Audience.WATER_CONSERVATION: "Water help",
    Audience.HOME_SECURITY: "Home security help",
    Audience.CYBER_HYGIENE: "Cyber hygiene help",
    Audience.PASSWORD_SECURITY: "Password help",
    Audience.BROWSER_EXT: "Browser ext help",
    Audience.EMAIL_PRODUCTIVITY: "Email productivity help",
    Audience.NOTE_TAKING: "Note-taking help",
    Audience.SPEED_READING: "Speed reading help",
    Audience.DEBATE: "Debate help",
    Audience.PUBLIC_POLICY_ANALYSIS: "Policy analysis help",
    Audience.MAPS_GIS: "GIS help",
    Audience.CARTOGRAPHY: "Cartography help",
    Audience.ASTROPHOTOGRAPHY: "Astrophotography help",
    Audience.METEOROLOGY_HOBBY: "Weather hobby help",
    Audience.AMATEUR_ASTRONOMY: "Amateur astronomy help",
    Audience.BOARD_GAMES: "Board games help",
    Audience.PUZZLES: "Puzzles help",
    Audience.RUBIKS: "Cubing help",
    Audience.ORIGAMI: "Origami help",
    Audience.KNIFE_SKILLS: "Knife skills help",
    Audience.MEAL_PREP: "Meal prep help",
    Audience.KETO: "Keto help",
    Audience.VEGAN_COOKING: "Vegan cooking help",
    Audience.GLUTEN_FREE: "Gluten-free help",
    Audience.SOUS_VIDE: "Sous vide help",
    Audience.SMOKING_MEAT: "Smoking meat help",
    Audience.PICKLEBALL: "Pickleball help",
    Audience.BADMINTON: "Badminton help",
    Audience.TABLE_TENNIS: "Table tennis help",
    Audience.RUGBY: "Rugby help",
    Audience.CRICKET: "Cricket help",
    Audience.SOFTBALL: "Softball help",
    Audience.LACROSSE: "Lacrosse help",
    Audience.WATER_POLO: "Water polo help",
    Audience.DIVING_SPORT: "Diving sport help",
    Audience.SYNCHRONIZED_SWIM: "Artistic swim help",
    Audience.EQUESTRIAN_SPORT: "Equestrian sport help",
    Audience.ESPORTS: "Esports help",
    Audience.SPEEDRUNNING: "Speedrun help",
    Audience.YOGA_THERAPY: "Yoga therapy help",
    Audience.MOBILITY: "Mobility help",
    Audience.BREATHWORK: "Breathwork help",
    Audience.SPANISH: "Spanish help",
    Audience.FRENCH: "French help",
    Audience.GERMAN: "German help",
    Audience.JAPANESE: "Japanese help",
    Audience.MANDARIN: "Mandarin help",
    Audience.KOREAN: "Korean help",
    Audience.ITALIAN: "Italian help",
    Audience.PORTUGUESE: "Portuguese help",
    Audience.ARABIC: "Arabic help",
    Audience.HINDI: "Hindi help",
    Audience.GREEK_LANG: "Greek language help",
    Audience.LATIN: "Latin help",
    Audience.UKULELE: "Ukulele help",
    Audience.SAXOPHONE: "Saxophone help",
    Audience.TRUMPET: "Trumpet help",
    Audience.FLUTE: "Flute help",
    Audience.HARMONICA: "Harmonica help",
    Audience.BANJO: "Banjo help",
    Audience.MUSIC_THEORY: "Music theory help",
    Audience.GRAPHIC_DESIGN: "Graphic design help",
    Audience.ILLUSTRATION: "Illustration help",
    Audience.UX_WRITING: "UX writing help",
    Audience.STORYBOARD: "Storyboard help",
    Audience.COLOR_GRADING: "Color grading help",
    Audience.LIGHTING_DESIGN: "Lighting help",
    Audience.COSTUME_DESIGN: "Costume help",
    Audience.SET_DESIGN: "Set design help",
    Audience.PASTRY: "Pastry help",
    Audience.BREAD: "Bread help",
    Audience.CHOCOLATE: "Chocolate help",
    Audience.CHEESE: "Cheese help",
    Audience.CHARCUTERIE: "Charcuterie help",
    Audience.PRESERVING: "Preserving help",
    Audience.INDIAN_COOKING: "Indian cooking help",
    Audience.CHINESE_COOKING: "Chinese cooking help",
    Audience.MEXICAN_COOKING: "Mexican cooking help",
    Audience.ITALIAN_COOKING: "Italian cooking help",
    Audience.JAPANESE_COOKING: "Japanese cooking help",
    Audience.BBQ_SAUCES: "BBQ sauce help",
    Audience.COFFEE_ROASTING: "Coffee roasting help",
    Audience.LATTE_ART: "Latte art help",
    Audience.HOUSEPLANTS: "Houseplants help",
    Audience.HYDROPONICS: "Hydroponics help",
    Audience.BONSAI: "Bonsai help",
    Audience.AQUAPONICS: "Aquaponics help",
    Audience.LAWN_CARE: "Lawn care help",
    Audience.IRRIGATION: "Irrigation help",
    Audience.POOL_CARE: "Pool care help",
    Audience.FIREPLACE: "Fireplace help",
    Audience.DENTAL_HYGIENE: "Dental hygiene help",
    Audience.PHARMACOLOGY: "Pharmacology help",
    Audience.RADIOLOGY_LITERACY: "Radiology help",
    Audience.NUTRITION_SCIENCE: "Nutrition science help",
    Audience.EPIDEMIOLOGY: "Epidemiology help",
    Audience.BIOSTATISTICS: "Biostats help",
    Audience.BOOKKEEPING: "Bookkeeping help",
    Audience.PAYROLL: "Payroll help",
    Audience.BILLING: "Billing help",
    Audience.PRICING: "Pricing help",
    Audience.SALES_ENABLEMENT: "Sales enablement help",
    Audience.PARTNERSHIPS: "Partnerships help",
    Audience.CUSTOMER_RESEARCH: "Customer research help",
    Audience.ANALYTICS: "Analytics help",
    Audience.AB_TESTING: "A/B testing help",
    Audience.RUST_LANG: "Rust help",
    Audience.GO_LANG: "Go help",
    Audience.PYTHON_DATA: "Python data help",
    Audience.SQL_ANALYTICS: "SQL analytics help",
    Audience.TERRAFORM: "Terraform help",
    Audience.ANSIBLE: "Ansible help",
    Audience.CICD: "CI/CD help",
    Audience.DOCKER: "Docker help",
    Audience.LINUX_ADMIN: "Linux admin help",
    Audience.NETWORK_SECURITY: "Network security help",
    Audience.PENTEST_DEFENSE: "Pentest-aware help",
    Audience.THREAT_MODEL: "Threat model help",
    Audience.INCIDENT_RESPONSE: "IR help",
    Audience.QA_TESTING: "QA testing help",
    Audience.MOBILE_QA: "Mobile QA help",
    Audience.ACCESSIBILITY_ENG: "A11y eng help",
    Audience.PERFORMANCE_WEB: "Web performance help",
    Audience.SEO_TECHNICAL: "Technical SEO help",
    Audience.TAX_PREP: "Tax prep help",
    Audience.INSURANCE_CLAIMS: "Claims help",
    Audience.CAR_BUYING: "Car buying help",
    Audience.HOME_BUYING: "Home buying help",
    Audience.RENTING: "Renting help",
    Audience.COLLEGE_APPS: "College apps help",
    Audience.SCHOLARSHIPS: "Scholarships help",
    Audience.STUDY_ABROAD: "Study abroad help",
    Audience.INTERNSHIP: "Internship help",
    Audience.CAREER_CHANGE: "Career change help",
    Audience.LINKEDIN: "LinkedIn help",
    Audience.WHATSAPP: "WhatsApp help",
    Audience.NETWORKING_CAREER: "Career networking help",
    Audience.HOA_LIVING: "HOA help",
    Audience.COOP_HOUSING: "Co-op housing help",
    Audience.COMMUNITY_GARDEN: "Community garden help",
    Audience.MUTUAL_AID: "Mutual aid help",
    Audience.FISHKEEPING: "Fishkeeping help",
    Audience.TERRARIUM: "Terrarium help",
    Audience.ANTKEEPING: "Antkeeping help",
    Audience.BEEKEEPING_ADVANCED: "Advanced beekeeping help",
    Audience.FOUNTAIN_PEN: "Fountain pen help",
    Audience.STATIONERY: "Stationery help",
    Audience.MECHANICAL_KEYBOARD: "Mech keyboard help",
    Audience.PC_BUILDING: "PC building help",
    Audience.HOME_LAB: "Homelab help",
    Audience.THREE_D_MODELING: "3D modeling help",
    Audience.CNC: "CNC help",
    Audience.LASER_CUTTING: "Laser cutting help",
    Audience.RESIN_PRINTING: "Resin printing help",
    Audience.FILAMENT_PRINTING: "FDM printing help",
    Audience.MEDITATION: "Meditation help",
    Audience.STOICISM: "Stoicism help",
    Audience.JOURNAL_PROMPTS: "Journal prompts help",
    Audience.HABIT_BUILDING: "Habit help",
    Audience.TIME_BLOCKING: "Time blocking help",
    Audience.SECOND_BRAIN: "Second brain help",
    Audience.PACKING: "Packing help",
    Audience.TRAVEL_PHOTOGRAPHY: "Travel photo help",
    Audience.SOLO_TRAVEL: "Solo travel help",
    Audience.FAMILY_TRAVEL: "Family travel help",
    Audience.BUDGET_TRAVEL: "Budget travel help",
    Audience.POINTS_MILES: "Points & miles help",
    Audience.REAL_ESTATE_PHOTO: "RE photography help",
    Audience.STAGING: "Staging help",
    Audience.INTERIOR_STYLING: "Interior styling help",
    Audience.EVENT_PHOTOGRAPHY: "Event photo help",
    Audience.PORTRAIT_PHOTO: "Portrait photo help",
    Audience.STREET_PHOTO: "Street photo help",
    Audience.WILDLIFE_PHOTO: "Wildlife photo help",
    Audience.ASTRO_IMAGING_PROC: "Astro processing help",
    Audience.PLAIN: "Here’s your help",
}


def audience_chip_label(audience: Audience | object) -> str:
    """Short chip for the Answers UI."""
    return _CHIP_LABELS.get(_coerce_audience(audience), "Keeping it simple")


def audience_chip_hint(audience: Audience | object) -> str:
    """One-line explanation under the chip."""
    return _CHIP_HINTS.get(_coerce_audience(audience), "Plain language only.")


def audience_header_title(audience: Audience | object) -> str:
    """Window header title for this mode."""
    return _HEADER_TITLES.get(_coerce_audience(audience), "Here’s your help")


def audience_css_class(audience: Audience | object) -> str:
    """CSS class name for the Answers chip."""
    aud = _coerce_audience(audience)
    if aud is Audience.CODE:
        return "teddyos-audience-code"
    if aud is Audience.PLAIN:
        return "teddyos-audience-plain"
    return f"teddyos-audience-{aud.value}"


def is_code_audience(audience: object) -> bool:
    """True when audience is CODE (enum or string)."""
    return _coerce_audience(audience) is Audience.CODE


def _coerce_audience(audience: object) -> Audience:
    if isinstance(audience, Audience):
        return audience
    val = getattr(audience, "value", audience)
    try:
        return Audience(str(val))
    except ValueError:
        return Audience.PLAIN
