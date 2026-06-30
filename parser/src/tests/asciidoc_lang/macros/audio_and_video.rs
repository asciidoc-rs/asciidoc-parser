use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/audio-and-video.adoc");

non_normative!(
    r#"
= Audio and Video
:url-video-element: https://developer.mozilla.org/en-US/docs/Web/HTML/Element/video
:url-audio-element: https://developer.mozilla.org/en-US/docs/Web/HTML/Element/audio
:url-media-formats: https://developer.mozilla.org/en-US/docs/Web/HTML/Supported_media_formats#Browser_compatibility

"#
);

mod audio_macro_syntax {
    use crate::{blocks::MediaType, tests::prelude::*};

    non_normative!(
        r#"
== Audio macro syntax

The block audio macro enables you to embed audio streams into your documentation.
You can embed self-hosted audio files that are supported by the browser.

The audio formats AsciiDoc supports is dictated by the output format, such as the formats supported by the browser when generating HTML.
While this was once a precarious ordeal, HTML 5 has brought sanity to audio support in the browser by adding a dedicated {url-audio-element}[`<audio>`^] element and by introducing several standard audio formats.
Those formats are now widely supported across browsers and systems.

For a canonical list of supported web audio formats and their interaction with modern browsers, see the {url-media-formats}[Mozilla Developer Supported Media Formats^] documentation.

"#
    );

    #[test]
    fn basic_audio_file_include() {
        verifies!(
            r#"
.Basic audio file include
----
include::example$audio.adoc[tag=basic]
----

"#
        );

        // example$audio.adoc[tag=basic]
        let doc = Parser::default().parse("audio::ocean-waves.wav[]");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Media(MediaBlock {
                    type_: MediaType::Audio,
                    target: Span {
                        data: "ocean-waves.wav",
                        line: 1,
                        col: 8,
                        offset: 7,
                    },
                    macro_attrlist: Attrlist {
                        attributes: &[],
                        anchor: None,
                        source: Span {
                            data: "",
                            line: 1,
                            col: 24,
                            offset: 23,
                        },
                    },
                    source: Span {
                        data: "audio::ocean-waves.wav[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "audio::ocean-waves.wav[]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn set_attributes_for_local_audio_playback() {
        verifies!(
            r#"
You can control the audio settings using additional attributes on the macro.
For instance, you can offset the start time of playback using the `start` attribute and enable autoplay using the `autoplay` option.

.Set attributes for local audio playback
----
include::example$audio.adoc[tag=attrs]
----

"#
        );

        // example$audio.adoc[tag=attrs]
        let doc = Parser::default().parse("audio::ocean-waves.wav[start=60,opts=autoplay]");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Media(MediaBlock {
                    type_: MediaType::Audio,
                    target: Span {
                        data: "ocean-waves.wav",
                        line: 1,
                        col: 8,
                        offset: 7,
                    },
                    macro_attrlist: Attrlist {
                        attributes: &[
                            ElementAttribute {
                                name: Some("start"),
                                value: "60",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: Some("opts"),
                                value: "autoplay",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "start=60,opts=autoplay",
                            line: 1,
                            col: 24,
                            offset: 23,
                        },
                    },
                    source: Span {
                        data: "audio::ocean-waves.wav[start=60,opts=autoplay]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "audio::ocean-waves.wav[start=60,opts=autoplay]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn add_a_caption_to_the_audio() {
        verifies!(
            r#"
You can include a caption above the audio using the title attribute.

.Add a caption to the audio
[source]
----
include::example$audio.adoc[tag=caption]
----

"#
        );

        // example$audio.adoc[tag=caption]
        let doc = Parser::default().parse(".Take a zen moment\naudio::ocean-waves.wav[]");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Media(MediaBlock {
                    type_: MediaType::Audio,
                    target: Span {
                        data: "ocean-waves.wav",
                        line: 2,
                        col: 8,
                        offset: 26,
                    },
                    macro_attrlist: Attrlist {
                        attributes: &[],
                        anchor: None,
                        source: Span {
                            data: "",
                            line: 2,
                            col: 24,
                            offset: 42,
                        },
                    },
                    source: Span {
                        data: ".Take a zen moment\naudio::ocean-waves.wav[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: Some(Span {
                        data: "Take a zen moment",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },),
                    title: Some("Take a zen moment"),
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: ".Take a zen moment\naudio::ocean-waves.wav[]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }
}

mod video_macro_syntax {
    use crate::{blocks::MediaType, tests::prelude::*};

    non_normative!(
        r#"
== Video macro syntax

The block video macro enables you to embed videos into your documentation.
You can embed self-hosted videos or videos shared on popular video hosting sites such as Vimeo and YouTube.

The video formats AsciiDoc supports is dictated by the output format, such as the formats supported by the browser when generating HTML.
While this was once a precarious ordeal, HTML 5 has brought sanity to video support in the browser by adding a dedicated {url-video-element}[`<video>`^] element and by introducing several standard video formats.
Those formats are now widely supported across browsers and systems.

For a canonical list of supported web video formats and their interaction with modern browsers, see the {url-media-formats}[Mozilla Developer Supported Media Formats^] documentation.

.A recommendation for serving video to browsers
****
Where appropriate, we recommend using a video hosting service like Vimeo or YouTube to serve videos in online documentation.
These services specialize in streaming optimized video to the browser, with the lowest latency possible given hardware, software, and network capabilities of the device viewing the video.

Vimeo even offers a white label mode so users aren't made aware that the video is being served through its service.

See <<Vimeo and YouTube videos>> for details about how to serve videos from these services.
****

"#
    );

    #[test]
    fn basic_video_file_include() {
        verifies!(
            r#"
.Basic video file include
[source]
----
include::example$video.adoc[tag=base]
----

"#
        );

        // example$video.adoc[tag=base]
        let doc = Parser::default().parse("video::video-file.mp4[]");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Media(MediaBlock {
                    type_: MediaType::Video,
                    target: Span {
                        data: "video-file.mp4",
                        line: 1,
                        col: 8,
                        offset: 7,
                    },
                    macro_attrlist: Attrlist {
                        attributes: &[],
                        anchor: None,
                        source: Span {
                            data: "",
                            line: 1,
                            col: 23,
                            offset: 22,
                        },
                    },
                    source: Span {
                        data: "video::video-file.mp4[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "video::video-file.mp4[]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn set_attributes_for_local_video_playback() {
        verifies!(
            r#"
You can control the video settings using additional attributes on the macro.
For instance, you can offset the start time of playback using the `start` attribute and enable autoplay using the `autoplay` option.

.Set attributes for local video playback
[source]
----
include::example$video.adoc[tag=attr]
----

"#
        );

        // example$video.adoc[tag=attr]
        let doc =
            Parser::default().parse("video::video-file.mp4[width=640,start=60,opts=autoplay]");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Media(MediaBlock {
                    type_: MediaType::Video,
                    target: Span {
                        data: "video-file.mp4",
                        line: 1,
                        col: 8,
                        offset: 7,
                    },
                    macro_attrlist: Attrlist {
                        attributes: &[
                            ElementAttribute {
                                name: Some("width"),
                                value: "640",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: Some("start"),
                                value: "60",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: Some("opts"),
                                value: "autoplay",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "width=640,start=60,opts=autoplay",
                            line: 1,
                            col: 23,
                            offset: 22,
                        },
                    },
                    source: Span {
                        data: "video::video-file.mp4[width=640,start=60,opts=autoplay]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "video::video-file.mp4[width=640,start=60,opts=autoplay]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn add_a_caption_to_a_video() {
        verifies!(
            r#"
You can include a caption on the video using the title attribute.

.Add a caption to a video
[source]
----
include::example$video.adoc[tag=caption]
----

"#
        );

        // example$video.adoc[tag=caption]
        let doc = Parser::default().parse(".A walkthrough of the product\nvideo::video-file.mp4[]");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Media(MediaBlock {
                    type_: MediaType::Video,
                    target: Span {
                        data: "video-file.mp4",
                        line: 2,
                        col: 8,
                        offset: 37,
                    },
                    macro_attrlist: Attrlist {
                        attributes: &[],
                        anchor: None,
                        source: Span {
                            data: "",
                            line: 2,
                            col: 23,
                            offset: 52,
                        },
                    },
                    source: Span {
                        data: ".A walkthrough of the product\nvideo::video-file.mp4[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: Some(Span {
                        data: "A walkthrough of the product",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },),
                    title: Some("A walkthrough of the product"),
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: ".A walkthrough of the product\nvideo::video-file.mp4[]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }
}

mod vimeo_and_youtube_videos {
    use crate::{
        blocks::{Block, MediaType},
        tests::prelude::*,
    };

    non_normative!(
        r#"
=== Vimeo and YouTube videos

The video macro supports embedding videos from external video hosting services like Vimeo and YouTube.
The AsciiDoc processor, specifically the converter, automatically generates the correct code to embed the video in the HTML output.

IMPORTANT: In order for an embedded YouTube video to work in Firefox when viewing the generated HTML document through the file: protocol, you must set `security.fileuri.strict_origin_policy` on the about:config settings page to `false`.

"#
    );

    #[test]
    fn embed_a_vimeo_video() {
        verifies!(
            r#"
To use this feature, put the video ID in the macro target and the name of the hosting service in the first positional attribute.

.Embed a Vimeo video
[source]
----
include::example$video.adoc[tag=vimeo]
----

"#
        );

        // The video ID is the macro target and the hosting service name (`vimeo`)
        // is the first positional attribute. How the target ID and service name
        // are assembled into an embed URL is the converter's responsibility, not
        // the parser's, so the parser simply preserves both verbatim.
        //
        // example$video.adoc[tag=vimeo]
        let doc = Parser::default().parse("video::67480300[vimeo]");

        let Some(Block::Media(media)) = doc.nested_blocks().next() else {
            panic!("expected a media block");
        };

        assert_eq!(media.type_(), MediaType::Video);
        assert_eq!(media.target().unwrap().data(), "67480300");

        let macro_attrlist = media.macro_attrlist();
        assert_eq!(macro_attrlist.nth_attribute(1).unwrap().value(), "vimeo");
    }

    #[test]
    fn embed_a_youtube_video() {
        verifies!(
            r#"
.Embed a YouTube video
[source]
----
include::example$video.adoc[tag=youtube]
----

"#
        );

        // example$video.adoc[tag=youtube]
        let doc = Parser::default().parse("video::RvRhUHTV_8k[youtube]");

        let Some(Block::Media(media)) = doc.nested_blocks().next() else {
            panic!("expected a media block");
        };

        assert_eq!(media.type_(), MediaType::Video);
        assert_eq!(media.target().unwrap().data(), "RvRhUHTV_8k");

        let macro_attrlist = media.macro_attrlist();
        assert_eq!(macro_attrlist.nth_attribute(1).unwrap().value(), "youtube");
    }

    #[test]
    fn embed_a_youtube_video_with_a_playlist() {
        verifies!(
            r#"
When embedding a YouTube video, you can specify a playlist to associate with the video using the `list` attribute.
The playlist must be specified by its ID.

.Embed a YouTube video with a playlist
[source]
----
include::example$video.adoc[tag=youtube-with-list]
----

"#
        );

        // The `list` named attribute carries the playlist ID. The parser exposes
        // it as a plain named attribute; mapping it onto the embed URL is the
        // converter's job.
        //
        // example$video.adoc[tag=youtube-with-list]
        let doc = Parser::default()
            .parse("video::RvRhUHTV_8k[youtube,list=PLDitloyBcHOm49bxNhvGgg0f9NRZ5lSaP]");

        let Some(Block::Media(media)) = doc.nested_blocks().next() else {
            panic!("expected a media block");
        };

        assert_eq!(media.target().unwrap().data(), "RvRhUHTV_8k");

        let macro_attrlist = media.macro_attrlist();
        assert_eq!(macro_attrlist.nth_attribute(1).unwrap().value(), "youtube");
        assert_eq!(
            macro_attrlist.named_attribute("list").unwrap().value(),
            "PLDitloyBcHOm49bxNhvGgg0f9NRZ5lSaP"
        );
    }

    #[test]
    fn embed_a_youtube_video_with_a_playlist_in_the_target() {
        verifies!(
            r#"
Instead of using the `list` attribute, you can specify the ID of the playlist after the video ID in the target, separated by a slash.

.Embed a YouTube video with a playlist in the target
[source]
----
include::example$video.adoc[tag=youtube-with-list-in-target]
----

"#
        );

        // The playlist ID follows the video ID in the target, separated by a
        // slash. The parser keeps the whole target verbatim (it does not split on
        // the slash); the converter is responsible for separating the video ID
        // from the playlist ID.
        //
        // example$video.adoc[tag=youtube-with-list-in-target]
        let doc = Parser::default()
            .parse("video::RvRhUHTV_8k/PLDitloyBcHOm49bxNhvGgg0f9NRZ5lSaP[youtube]");

        let Some(Block::Media(media)) = doc.nested_blocks().next() else {
            panic!("expected a media block");
        };

        assert_eq!(
            media.target().unwrap().data(),
            "RvRhUHTV_8k/PLDitloyBcHOm49bxNhvGgg0f9NRZ5lSaP"
        );

        let macro_attrlist = media.macro_attrlist();
        assert_eq!(macro_attrlist.nth_attribute(1).unwrap().value(), "youtube");
    }

    #[test]
    fn embed_a_youtube_video_with_a_dynamic_playlist() {
        verifies!(
            r#"
Alternatively, you can create a dynamic, unnamed playlist by listing several additional video IDs in the `playlist` attribute.

.Embed a YouTube video with a dynamic playlist
[source]
----
include::example$video.adoc[tag=youtube-with-playlist]
----

"#
        );

        // The `playlist` named attribute holds a comma-separated list of video
        // IDs. Because the value contains commas, it is enclosed in double quotes
        // so the attribute parser keeps it as a single value.
        //
        // example$video.adoc[tag=youtube-with-playlist]
        let doc = Parser::default()
            .parse("video::RvRhUHTV_8k[youtube,playlist=\"_SvwdK_HibQ,SGqg_ZzThDU\"]");

        let Some(Block::Media(media)) = doc.nested_blocks().next() else {
            panic!("expected a media block");
        };

        assert_eq!(media.target().unwrap().data(), "RvRhUHTV_8k");

        let macro_attrlist = media.macro_attrlist();
        assert_eq!(macro_attrlist.nth_attribute(1).unwrap().value(), "youtube");
        assert_eq!(
            macro_attrlist.named_attribute("playlist").unwrap().value(),
            "_SvwdK_HibQ,SGqg_ZzThDU"
        );
    }

    #[test]
    fn embed_a_youtube_video_with_a_dynamic_playlist_in_the_target() {
        verifies!(
            r#"
Instead of using the `playlist` attribute, you can create a dynamic, unnamed playlist by listing several video IDs in the target separated by a comma.

.Embed a YouTube video with a dynamic playlist in the target
[source]
----
include::example$video.adoc[tag=youtube-with-playlist-in-target]
----

"#
        );

        // The additional video IDs follow the first video ID in the target,
        // separated by commas. The parser keeps the entire comma-separated target
        // verbatim; splitting it into the lead video ID and the playlist members
        // is the converter's job.
        //
        // example$video.adoc[tag=youtube-with-playlist-in-target]
        let doc = Parser::default().parse("video::RvRhUHTV_8k,_SvwdK_HibQ,SGqg_ZzThDU[youtube]");

        let Some(Block::Media(media)) = doc.nested_blocks().next() else {
            panic!("expected a media block");
        };

        assert_eq!(
            media.target().unwrap().data(),
            "RvRhUHTV_8k,_SvwdK_HibQ,SGqg_ZzThDU"
        );

        let macro_attrlist = media.macro_attrlist();
        assert_eq!(macro_attrlist.nth_attribute(1).unwrap().value(), "youtube");
    }
}

mod audio_and_video_attributes_and_options {
    use crate::tests::prelude::*;

    non_normative!(
        r#"
== Audio and video attributes and options

.Audio attributes and values
[%autowidth]
|===
|Attribute |Value(s) |Example Syntax |Notes

|`title`
|User defined text
|`.Ocean waves`
|

|`start`
|User-defined playback start time in seconds.
|`start=30`
|

|`end`
|User-defined playback end time in seconds.
|`end=90`
|

|`options` (`opts`)
|`autoplay`, `loop`, `controls`, `nocontrols`
|`opts="autoplay,loop"`
|The controls value is enabled by default
|===

.Video attributes and values
[%autowidth]
|===
|Attribute |Value(s) |Example Syntax |Notes

|`title`
|User defined text
|`.An ocean sunset`
|

|`poster`
|A URL to an image to show until the user plays or seeks.
|`poster=sunset.jpg`
|Can be specified as the first positional (unnamed) attribute.
Also used to specify the service when referring to a video hosted on Vimeo (`vimeo`) or YouTube (`youtube`).

|`width`
|User-defined size in pixels.
|`width=640`
|Can be specified as the second positional (unnamed) attribute.

|`height`
|User-defined size in pixels.
|`height=480`
|Can be specified as the third positional (unnamed) attribute.

|`start`
|User-defined playback start time in seconds.
|`start=30`
|

|`end`
|User-defined playback end time in seconds.
|`end=90`
|

|`theme`
|The YouTube theme to use for the frame.
|`theme=light`
|Valid values are `dark` (the default) and `light`.

|`lang`
|The language used in the YouTube frame.
|`lang=fr`
|A BCP 47 language tag (typically a two-letter language code, like `en`).

|`list`
|The ID of a playlist to associate with a YouTube video.
|`list=PLabc123`
|Only applies to YouTube videos.

|`playlist`
|Additional video IDs to create a dynamic YouTube playlist.
|`playlist="video-abc,video-xyz"`
|IDs must be separated by commas.
Therefore, the value must be enclosed in double quotes.
Only applies to YouTube videos.

|`align`
|`left`, `center`, `right`
|`align=center`
|Follows the same alignment rules as a block image.

|`options` (`opts`)
|`autoplay`, `loop`, `modest`, `nocontrols`, `nofullscreen`, `muted`
|`opts="autoplay,loop"`
|The controls are enabled by default.
The `modest` option enables modest branding for a YouTube video.
|===
"#
    );
}
