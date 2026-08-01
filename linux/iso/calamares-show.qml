import QtQuick 2.0
import calamares.slideshow 1.0

/* The slideshow Calamares shows while it copies the filesystem.
 *
 * This exists because it has to. branding.desc declared `slideshowAPI: 2` and
 * omitted `slideshow:`, on the reasoning that an installer which gets on with
 * the job beats one that runs marketing at you. Calamares disagreed: an
 * inconsistent descriptor makes it refuse the whole branding component and exit
 * before drawing anything, so clicking Install teddyOS did nothing at all — no
 * window, no error, nothing.
 *
 * So: one slide, saying what is happening. Still no carousel.
 */
Presentation {
    id: presentation

    function onActivate() { }
    function onLeave() { }

    Slide {
        Text {
            anchors.centerIn: parent
            horizontalAlignment: Text.AlignHCenter
            text: "Installing teddyOS\n\nThis takes a few minutes.\nYou can leave it running."
            font.pixelSize: 20
            lineHeight: 1.4
            color: "#2c2c2a"
        }
    }
}
