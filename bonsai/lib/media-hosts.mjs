// The hosts a stand reads media from, and the roles a medium plays on a card.
//
// The stands' pages admit images and image-service descriptions from these hosts
// alone: the site Worker builds its Content-Security-Policy from this list, and
// the bonsai check refuses a media module that names any other host. A new
// provider joins the list together with its first media entry. A provider that
// serves files without CORS keeps no place here: its files reach the page as
// the stand's own copies, from the site itself.

/** HTTPS hosts of the image services the media modules name. */
export const MEDIA_HOSTS=Object.freeze([
  'iiif.micr.io', // Rijksmuseum IIIF Image API
  'api.nga.gov', // National Gallery of Art IIIF Image API
]);

/**
 * The hosts each provider serves its image files from, which the stand keeps as its own copies. The attach
 * command fetches a provider's files from these hosts alone.
 */
export const FILE_HOSTS=Object.freeze({
  met:Object.freeze(['images.metmuseum.org']),
  commons:Object.freeze(['thumb.wikimedia.org','upload.wikimedia.org']),
  slam:Object.freeze(['d23gjylftzdlen.cloudfront.net']),
  smb:Object.freeze(['search.smb.museum']),
});

/** What a medium is for on a card, in the order a card shows them. */
// A response to the book is a work made about it; a shared motif shows what the text names, in a work of its own.
export const MEDIA_ROLES=Object.freeze(['author-portrait','book-response','motif','era-material','object-of-analysis','comparison']);
