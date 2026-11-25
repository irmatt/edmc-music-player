import { useState } from "react";
import type { Forum, UiPreferences } from "../types";

interface LibraryTreeProps {
  tree: Forum[];
  prefs: UiPreferences;
  likedCount: number;
  likedSelected: boolean;
  selectedUrl: string | null;
  loadingChildren: Set<string>;
  onSelect: (forum: Forum) => void;
  onExpand: (forum: Forum) => void;
  onExpandAlways: (forum: Forum) => void;
  onToggleFavorite: (forum: Forum) => void;
  onReorderFavorite: (from: number, to: number) => void;
}

function flatten(forums: Forum[], output: Forum[] = []): Forum[] {
  for (const forum of forums) {
    output.push(forum);
    flatten(forum.children, output);
  }
  return output;
}

function Node({
  forum,
  depth,
  prefs,
  selectedUrl,
  loadingChildren,
  onSelect,
  onExpand,
  onExpandAlways,
  onToggleFavorite,
}: {
  forum: Forum;
  depth: number;
  prefs: UiPreferences;
  selectedUrl: string | null;
  loadingChildren: Set<string>;
  onSelect: (forum: Forum) => void;
  onExpand: (forum: Forum) => void;
  onExpandAlways: (forum: Forum) => void;
  onToggleFavorite: (forum: Forum) => void;
}) {
  const expanded = forum.children.length > 0 && !prefs.collapsed.includes(forum.url);
  const hasChildren = forum.children.length > 0 || depth < 2;
  const favorite = prefs.favorites.includes(forum.url);
  return (
    <div className="tree-node">
      <div
        className={`tree-row ${selectedUrl === forum.url ? "selected" : ""}`}
        style={{ paddingLeft: 8 + depth * 14 }}
      >
        <button
          className="tree-arrow"
          aria-label={expanded ? "Collapse" : "Expand"}
          disabled={!hasChildren || loadingChildren.has(forum.url)}
          onClick={() => onExpand(forum)}
        >
          {loadingChildren.has(forum.url) ? "…" : hasChildren ? (expanded ? "▾" : "▸") : ""}
        </button>
        <button className="tree-label" onClick={() => onSelect(forum)} onDoubleClick={() => onExpandAlways(forum)} title={forum.name}>
          {forum.name}
        </button>
        {depth > 0 && (
          <button
            className={`tree-star ${favorite ? "favorite" : ""}`}
            title={favorite ? "Remove from favourites" : "Pin to favourites"}
            onClick={() => onToggleFavorite(forum)}
          >
            {favorite ? "★" : "☆"}
          </button>
        )}
      </div>
      {expanded && forum.children.length > 0 && (
        <div className="tree-children">
          {[...forum.children]
            .sort((a, b) => a.name.localeCompare(b.name))
            .map((child) => (
            <Node
              key={child.url}
              forum={child}
              depth={depth + 1}
              prefs={prefs}
              selectedUrl={selectedUrl}
              loadingChildren={loadingChildren}
              onSelect={onSelect}
              onExpand={onExpand}
              onExpandAlways={onExpandAlways}
              onToggleFavorite={onToggleFavorite}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export function LibraryTree({
  tree,
  prefs,
  likedCount,
  likedSelected,
  selectedUrl,
  loadingChildren,
  onSelect,
  onExpand,
  onExpandAlways,
  onToggleFavorite,
  onReorderFavorite,
}: LibraryTreeProps) {
  const [drag, setDrag] = useState<number | null>(null);
  const all = flatten(tree);
  const favorites = prefs.favorites
    .map((url) => all.find((forum) => forum.url === url))
    .filter((forum): forum is Forum => Boolean(forum));

  return (
    <div className="library-tree">
      <div className="tree-section-title">Favourites</div>
      <div className="favorites-list">
        {favorites.length === 0 && <div className="tree-empty">Pin genres here with ☆</div>}
        {favorites.map((forum, index) => (
          <div
            key={forum.url}
            className={`favorite-row ${selectedUrl === forum.url ? "selected" : ""} ${drag === index ? "dragging" : ""}`}
            draggable
            onDragStart={() => setDrag(index)}
            onDragOver={(event) => event.preventDefault()}
            onDrop={() => {
              if (drag !== null && drag !== index) onReorderFavorite(drag, index);
              setDrag(null);
            }}
            onClick={() => onSelect(forum)}
          >
            <span>★</span>
            <span className="tree-label">{forum.name}</span>
          </div>
        ))}
      </div>
      <div className="tree-section-title">Like tracks</div>
      <div
        className={`liked-row ${likedSelected ? "selected" : ""}`}
        style={{ paddingLeft: 26 }}
        onClick={() =>
          onSelect({ name: "Liked Tracks", url: "liked://tracks", parent_url: null, children: [] })
        }
      >
        <span className="liked-star">♥</span>
        <span className="tree-label">Liked Tracks</span>
        <span className="liked-count">{likedCount}</span>
      </div>
      <div className="tree-section-title">Library</div>
      {[...tree]
        .sort((a, b) => a.name.localeCompare(b.name))
        .map((forum) => (
        <Node
          key={forum.url}
          forum={forum}
          depth={0}
          prefs={prefs}
          selectedUrl={selectedUrl}
          loadingChildren={loadingChildren}
          onSelect={onSelect}
          onExpand={onExpand}
          onExpandAlways={onExpandAlways}
          onToggleFavorite={onToggleFavorite}
        />
      ))}
    </div>
  );
}
