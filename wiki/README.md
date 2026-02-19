# FloPro Wiki Pages

This directory contains markdown files for the FloPro GitHub wiki.

## Publishing to GitHub Wiki

GitHub wikis are separate git repositories. To publish these pages:

### 1. Clone the Wiki Repository

```bash
# Clone your main repo's wiki
git clone https://github.com/treeherder/illinois_river_flood_warning.wiki.git
cd illinois_river_flood_warning.wiki
```

### 2. Copy Wiki Pages

```bash
# Copy all wiki markdown files
cp /path/to/flopro/wiki/*.md .
```

### 3. Commit and Push

```bash
git add *.md
git commit -m "Add comprehensive technical documentation wiki"
git push origin main
```

### 4. View Wiki

Navigate to: https://github.com/treeherder/illinois_river_flood_warning/wiki

**Note:** The `Home.md` file becomes the wiki home page.

## Wiki Structure

### Core Documentation
- **Home.md** - Overview and navigation
- **Data-Sources.md** - What we're monitoring (USGS NWIS, IV vs DV APIs)
- **Database-Architecture.md** - How we're storing it (PostgreSQL schema)
- **Staleness-Tracking.md** - How we keep it accurate (hybrid cache)
- **Technology-Stack.md** - Justifications for tech choices
- **Station-Registry.md** - Details on 8 monitored stations

### Future Pages (To Be Created)
- **Database-Setup.md** - PostgreSQL installation guide
- **Historical-Data-Ingestion.md** - Running the 87-year backfill
- **Real-Time-Monitoring.md** - Service configuration
- **API-Reference.md** - Internal API docs (future)
- **Deployment.md** - Production deployment guide

## Page Naming Convention

GitHub wiki converts filename to page title:
- `Data-Sources.md` → "Data Sources" 
- `Database-Architecture.md` → "Database Architecture"

Use hyphens for spaces, proper capitalization.

## Internal Links

Use wiki-style links in markdown:
```markdown
[[Page Title]]
[[Custom Text|Page-Title]]
```

GitHub automatically converts these to working wiki links.

## Editing Wiki

**Option 1: Web Interface**
- Navigate to wiki page
- Click "Edit" button
- Edit in browser

**Option 2: Git Clone (Recommended)**
- Clone wiki.git repository
- Edit locally
- Commit and push

**Option 3: Keep in Main Repo**
- Edit files in /wiki/ directory
- Manually sync to wiki.git periodically

## Wiki vs README

**README.md** (in main repo):
- Quick start guide
- Installation instructions
- Basic usage
- Status badges

**Wiki** (separate repo):
- Detailed technical documentation
- Architecture decisions
- Operational guides
- In-depth explanations

Both link to each other for navigation.
