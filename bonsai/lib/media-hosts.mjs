// The hosts a stand reads media from, and the roles a medium plays on a card.
//
// The stands' pages admit images and image-service descriptions from these hosts
// alone: the site Worker builds its Content-Security-Policy from this list, and
// the bonsai check refuses a media module that names any other host. A new
// provider joins the list together with its first media entry.

/** HTTPS hosts of the image services the media modules name. */
export const MEDIA_HOSTS=Object.freeze([
  'iiif.micr.io', // Rijksmuseum IIIF Image API
]);

/** What a medium is for on a card, in the order a card shows them. */
export const MEDIA_ROLES=Object.freeze(['author-portrait','era-material','object-of-analysis','comparison']);
