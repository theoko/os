// Hide quick-settings toggles teddyOS has no reason to offer.
//
// Written here rather than installed from extensions.gnome.org deliberately.
// The general-purpose tweakers do this and forty other things, and each brings
// its own preferences window — spending a new settings UI to remove two
// buttons is not a simplification. This is the whole feature:
//
//   Night Light   a colour-temperature schedule nobody configured
//   Dark Style    duplicates the Appearance panel, one click away
//
// Both are built into gnome-shell, so there is no package to remove and no
// dconf key to unset; hiding them is the only lever that exists.
//
// IMPORTANT, and the reason a first attempt did nothing while reporting
// State: ACTIVE — `quickSettings._nightLight` is a SystemIndicator, not the
// button. Since GNOME 43 the visible tile lives in its `quickSettingsItems`
// array, so setting `.visible = false` on the indicator hides an object that
// was never drawn and leaves the toggle exactly where it was.
//
// disable() restores them because an extension that cannot be turned off is
// not a setting, it is damage.

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import St from 'gi://St';
import Clutter from 'gi://Clutter';

const HIDE = ['_nightLight', '_darkMode', '_backgroundApps'];

// GNOME labels the network toggle by connection TYPE: "Wired", "Wi-Fi".
// That is the correct word for an engineer and the wrong one for everyone
// else — someone who wants to know whether they are online reads "Wired" and
// learns nothing about that. The connection type is not the question they are
// asking; whether the internet works is.
const RELABEL = new Map([
    ['Wired', 'Internet'],
    ['Wired Connected', 'Internet'],
]);

// The menu BEHIND the toggle is built separately, by NetworkManager, and says
// "Wired" three more times: a header, the connection row, and the settings
// link. Renaming only the toggle fixes the word someone sees first and leaves
// the word they see when they click it — which is worse than not renaming it,
// because now the two disagree.
const MENU_RELABEL = new Map([
    ['Wired Connections', 'Internet'],
    ['Wired Settings', 'Internet settings'],
    ['Wired', 'Connected'],
]);

export default class TeddyOSShell {
    enable() {
        this._hidden = [];
        this._signals = [];
        this._restore = [];
        this._menuRestore = [];
        const quick = Main.panel.statusArea.quickSettings;
        if (!quick)
            return;

        this._labelDock();
        this._relabel(quick);

        for (const key of HIDE) {
            const indicator = quick[key];
            // Guard every lookup: these are private fields, so a GNOME release
            // may rename or drop one. A missing key should cost that toggle,
            // not throw and take the shell down with it on a machine someone
            // is trying for the first time.
            if (!indicator)
                continue;

            const items = indicator.quickSettingsItems ?? [];
            for (const item of items) {
                item.visible = false;
                this._hidden.push(item);
            }
            // The indicator itself is the small status glyph in the panel; it
            // is usually already hidden, but hide it too so nothing is left
            // pointing at a toggle that is gone.
            if (indicator.visible) {
                indicator.visible = false;
                this._hidden.push(indicator);
            }
        }
    }

    // NetworkManager rewrites the toggle's title whenever the connection
    // changes, so setting it once is not enough — it reverts the first time
    // the cable state flaps. Re-apply on notify::title, with a guard, because
    // setting the property inside its own notify handler recurses forever and
    // takes the shell with it.
    _relabel(quick) {
        const network = quick._network;
        if (!network)
            return;

        for (const item of network.quickSettingsItems ?? []) {
            if (typeof item.title !== 'string')
                continue;

            const apply = () => {
                if (item._teddyosBusy)
                    return;
                const replacement = RELABEL.get(item.title);
                if (replacement) {
                    item._teddyosBusy = true;
                    item.title = replacement;
                    item._teddyosBusy = false;
                }
            };

            this._restore.push([item, item.title]);
            apply();
            this._signals.push([item, item.connect('notify::title', apply)]);

            const menu = item.menu;
            if (menu) {
                this._relabelMenu(menu);
                this._signals.push([menu, menu.connect('open-state-changed',
                    (_m, open) => {
                        if (open)
                            this._relabelMenu(menu);
                    })]);
            }
        }
    }

    // Walk the menu and rewrite any label NetworkManager wrote in its own
    // vocabulary. Done on open rather than once at enable() because the menu
    // is rebuilt whenever a connection appears or drops — a single pass at
    // startup relabels a menu that is thrown away on the first state change.
    _relabelMenu(menu) {
        const walk = (actor) => {
            if (!actor)
                return;
            // St.Label is the only thing carrying user-visible text here.
            if (actor.constructor?.name === 'St_Label' || 'text' in actor) {
                const current = actor.text;
                if (typeof current === 'string' && MENU_RELABEL.has(current)) {
                    if (!this._menuRestore.some(([a]) => a === actor))
                        this._menuRestore.push([actor, current]);
                    actor.text = MENU_RELABEL.get(current);
                }
            }
            for (const child of actor.get_children?.() ?? [])
                walk(child);
        };
        walk(menu.box ?? menu.actor);
    }


    // Names under the dock icons.
    //
    // Six tiles and no words. A magnifying glass, a spark, a pinwheel, a green
    // phone, a folder and a downward arrow are obvious to anyone who has used a
    // computer for a decade and a guessing game to the people this desktop is
    // for — and hovering to find out is a thing you only do if you already
    // suspect a tooltip exists.
    //
    // Dash to Dock has no persistent-label option; its labels are hover
    // tooltips. So the name is added as a child of each icon's own container,
    // which means it follows the icon when the dock moves, hides and rescales
    // with it, and needs no polling.
    //
    // Wrapped in try/catch per item rather than around the loop: these are a
    // third-party extension's internals, so a GNOME or Dash to Dock update can
    // rename any of them. Losing one label is a blemish; throwing here takes
    // the whole shell down on someone's first boot.
    _labelDock() {
        this._labels = [];
        const dash = Main.overview?.dash ?? Main.uiGroup.find_child_by_name?.('dashtodockContainer');
        const items = dash?._box?.get_children?.() ?? [];
        for (const item of items) {
            try {
                const app = item.child?._delegate?.app ?? item.child?.app;
                const name = app?.get_name?.();
                if (!name || item._teddyosLabel)
                    continue;
                const label = new St.Label({
                    text: name,
                    style_class: 'teddyos-dock-label',
                    // Small, dimmed, and centred under the tile. Loud enough to
                    // read at a glance, quiet enough that the icons stay the
                    // thing you look at.
                    style: 'font-size: 9pt; color: rgba(255,255,255,0.92); ' +
                           'text-align: center; padding-top: 2px;',
                    x_align: Clutter.ActorAlign.CENTER,
                });
                item.child?.add_child?.(label);
                item._teddyosLabel = label;
                this._labels.push([item, label]);
            } catch {
                // This tile keeps its tooltip and loses its label. Acceptable.
            }
        }
    }

    disable() {
        for (const [item, label] of this._labels ?? []) {
            try {
                label.destroy();
                delete item._teddyosLabel;
            } catch {
                // Already destroyed with the dock; nothing to undo.
            }
        }
        this._labels = null;

        for (const item of this._hidden ?? [])
            item.visible = true;
        this._hidden = null;

        for (const [actor, text] of this._menuRestore ?? []) {
            try {
                actor.text = text;
            } catch {
                // The actor may be destroyed already; restoring a label on a
                // dead object is not worth taking disable() down for.
            }
        }
        this._menuRestore = null;

        for (const [item, id] of this._signals ?? [])
            item.disconnect(id);
        this._signals = null;

        for (const [item, title] of this._restore ?? []) {
            item._teddyosBusy = true;
            item.title = title;
            item._teddyosBusy = false;
        }
        this._restore = null;
    }
}
