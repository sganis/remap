#include <gst/gst.h>

#ifdef __APPLE__
#include <TargetConditionals.h>
#endif


/* Structure to contain all our information, so we can pass it to callbacks */
typedef struct _CustomData {
  GstElement *pipeline;
  GstElement *source;
  GstElement *demuxer;
  GstElement *decoder;
  GstElement *sink;
} CustomData;

static void demuxer_pad_added_handler (GstElement *src, GstPad *pad, CustomData *data);

int tutorial_main (int argc, char *argv[])
{
    CustomData data;
    GstBus *bus;
    GstMessage *msg;
    GstStateChangeReturn ret;
    gboolean terminate = FALSE;

    /* Initialize GStreamer */
    gst_init (&argc, &argv);

    /* Create the elements */
    //   source = gst_element_factory_make ("videotestsrc", "source");
    //   sink = gst_element_factory_make ("autovideosink", "sink");
    //tcpclientsrc host=127.0.0.1 port=7001 ! multipartdemux ! jpegdec ! glimagesink
    data.source = gst_element_factory_make ("tcpclientsrc", "source");
    data.demuxer = gst_element_factory_make ("multipartdemux", "demuxer");
    data.decoder = gst_element_factory_make ("jpegdec", "decoder");
    data.sink = gst_element_factory_make ("glimagesink", "sink");

    /* Create the empty pipeline */
    data.pipeline = gst_pipeline_new ("test-pipeline");

    if (!data.pipeline || !data.source || !data.demuxer || !data.decoder 
        || !data.sink) {
        g_printerr ("Not all elements could be created.\n");
        return -1;
    }


    /* Modify the source's properties */
    //   g_object_set (source, "pattern", 0, NULL);
    g_object_set (data.source, "host", "127.0.0.1", NULL);
    g_object_set (data.source, "port", 7001, NULL);

    /* Build the pipeline */
    gst_bin_add_many (GST_BIN (data.pipeline), 
        data.source, data.demuxer, data.decoder, data.sink, NULL);

    if (gst_element_link (data.decoder, data.sink) != TRUE) {
        g_printerr ("Elements could not be linked: decoder->sink.\n");
        gst_object_unref (data.pipeline);
        return -1;
    }

    if (gst_element_link (data.source, data.demuxer) != TRUE) {
        g_printerr ("Elements could not be linked: source->demuxer.\n");
        gst_object_unref (data.pipeline);
        return -1;
    }

    /* Connect to the pad-added signal */
    g_signal_connect (data.demuxer, "pad-added", G_CALLBACK (demuxer_pad_added_handler), &data);



    /* Start playing */
    ret = gst_element_set_state (data.pipeline, GST_STATE_PLAYING);
    if (ret == GST_STATE_CHANGE_FAILURE) {
        g_printerr ("Unable to set the pipeline to the playing state.\n");
        gst_object_unref (data.pipeline);
        return -1;
    }

    /* Listen to the bus */
    bus = gst_element_get_bus (data.pipeline);
    do {
        msg = gst_bus_timed_pop_filtered (bus, GST_CLOCK_TIME_NONE,
            GST_MESSAGE_STATE_CHANGED | GST_MESSAGE_ERROR | GST_MESSAGE_EOS);

        /* Parse message */
        if (msg != NULL) {
            GError *err;
            gchar *debug_info;

            switch (GST_MESSAGE_TYPE (msg)) {
            case GST_MESSAGE_ERROR:
                gst_message_parse_error (msg, &err, &debug_info);
                g_printerr ("Error received from element %s: %s\n", 
                    GST_OBJECT_NAME (msg->src), err->message);
                g_printerr ("Debugging information: %s\n", 
                    debug_info ? debug_info : "none");
                g_clear_error (&err);
                g_free (debug_info);
                terminate = TRUE;
                break;
            case GST_MESSAGE_EOS:
                g_print ("End-Of-Stream reached.\n");
                terminate = TRUE;
                break;
            case GST_MESSAGE_STATE_CHANGED:
                /* We are only interested in state-changed messages from the pipeline */
                if (GST_MESSAGE_SRC (msg) == GST_OBJECT (data.pipeline)) {
                GstState old_state, new_state, pending_state;
                gst_message_parse_state_changed (msg, &old_state, &new_state, &pending_state);
                g_print ("Pipeline state changed from %s to %s:\n",
                    gst_element_state_get_name (old_state), gst_element_state_get_name (new_state));
                }
                break;
            default:
                /* We should not reach here */
                g_printerr ("Unexpected message received.\n");
                break;
            }
            gst_message_unref (msg);
        }
    } while (!terminate);

    /* Free resources */
    gst_object_unref (bus);
    gst_element_set_state (data.pipeline, GST_STATE_NULL);
    gst_object_unref (data.pipeline);
    return 0;
}

int main (int argc, char *argv[])
{
#if defined(__APPLE__) && TARGET_OS_MAC && !TARGET_OS_IPHONE
  return gst_macos_main (tutorial_main, argc, argv, NULL);
#else
  return tutorial_main (argc, argv);
#endif
}

static void demuxer_pad_added_handler (GstElement *src, GstPad *new_pad, CustomData *data) {
    g_print("demuxer signal recieved\n");
    GstPad *sink_pad = gst_element_get_static_pad (data->decoder, "sink");
    GstPadLinkReturn ret;
    GstCaps *new_pad_caps = NULL;
    GstStructure *new_pad_struct = NULL;
    const gchar *new_pad_type = NULL;

    g_print ("Received new pad '%s' from '%s':\n", 
        GST_PAD_NAME (new_pad), GST_ELEMENT_NAME (src));

    /* If our converter is already linked, we have nothing to do here */
    if (gst_pad_is_linked (sink_pad)) {
        g_print ("We are already linked. Ignoring.\n");
        goto exit;
    }

    /* Check the new pad's type */
    new_pad_caps = gst_pad_get_current_caps (new_pad);
    new_pad_struct = gst_caps_get_structure (new_pad_caps, 0);
    new_pad_type = gst_structure_get_name (new_pad_struct);
    if (!g_str_has_prefix (new_pad_type, "image/jpeg")) {
        g_print ("It has type '%s' which is not image. Ignoring.\n", new_pad_type);
        goto exit;
    }

    /* Attempt the link */
    ret = gst_pad_link (new_pad, sink_pad);
    if (GST_PAD_LINK_FAILED (ret)) {
    g_print ("Type is '%s' but link failed.\n", new_pad_type);
    } else {
    g_print ("Link succeeded (type '%s').\n", new_pad_type);
    }

exit:
    /* Unreference the new pad's caps, if we got them */
    if (new_pad_caps != NULL)
    gst_caps_unref (new_pad_caps);

    /* Unreference the sink pad */
    gst_object_unref (sink_pad);
}
