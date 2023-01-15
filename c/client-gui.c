#include <string.h>

#include <gtk/gtk.h>
#include <gst/gst.h>
#include <gst/video/videooverlay.h>

#include <gdk/gdk.h>
#if defined (GDK_WINDOWING_X11)
#include <gdk/gdkx.h>
#elif defined (GDK_WINDOWING_WIN32)
#include <gdk/gdkwin32.h>
#elif defined (GDK_WINDOWING_QUARTZ)
#include <gdk/gdkquartz.h>
#endif

/* Structure to contain all our information, so we can pass it around */
typedef struct _CustomData {
  GstElement *pipeline;
  GstElement *source;
  GstElement *demuxer;
  GstElement *decoder;
  GstElement *sink;
  GstState state;                 /* Current state of the pipeline */
  gint64 duration;                /* Duration of the clip, in nanoseconds */
} CustomData;


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


/* This function is called when the GUI toolkit creates the physical window that will hold the video.
 * At this point we can retrieve its handler (which has a different meaning depending on the windowing system)
 * and pass it to GStreamer through the VideoOverlay interface. */
static void realize_cb (GtkWidget *widget, CustomData *data) {
  GdkWindow *window = gtk_widget_get_window (widget);
  guintptr window_handle;

  if (!gdk_window_ensure_native (window))
    g_error ("Couldn't create native window needed for GstVideoOverlay!");

  /* Retrieve window handler from GDK */
#if defined (GDK_WINDOWING_WIN32)
  window_handle = (guintptr)GDK_WINDOW_HWND (window);
#elif defined (GDK_WINDOWING_QUARTZ)
  window_handle = gdk_quartz_window_get_nsview (window);
                  //gdk_quartz_window_get_nsview
#elif defined (GDK_WINDOWING_X11)
  window_handle = GDK_WINDOW_XID (window);
#endif
  /* Pass it to pipeline, which implements VideoOverlay and will forward it to the video sink */
  gst_video_overlay_set_window_handle (GST_VIDEO_OVERLAY (data->sink), window_handle);
}

/* This function is called when the main window is closed */
static void delete_event_cb (GtkWidget *widget, GdkEvent *event, CustomData *data) {
  gtk_main_quit ();
}

/* This function is called everytime the video window needs to be redrawn (due to damage/exposure,
 * rescaling, etc). GStreamer takes care of this in the PAUSED and PLAYING states, otherwise,
 * we simply draw a black rectangle to avoid garbage showing up. */
static gboolean draw_cb (GtkWidget *widget, cairo_t *cr, CustomData *data) {
  if (data->state < GST_STATE_PAUSED) {
    GtkAllocation allocation;

    /* Cairo is a 2D graphics library which we use here to clean the video window.
     * It is used by GStreamer for other reasons, so it will always be available to us. */
    gtk_widget_get_allocation (widget, &allocation);
    cairo_set_source_rgb (cr, 0, 0, 0);
    cairo_rectangle (cr, 0, 0, allocation.width, allocation.height);
    cairo_fill (cr);
  }

  return FALSE;
}

static gboolean key_press_cb (GtkWidget *widget, GdkEventKey *event, 
    gpointer data) {
    g_print("key press: %d\n", event->keyval);
    if (event->keyval == GDK_KEY_space){
        g_print("SPACE KEY PRESSED!\n");
        return TRUE;
    }
    return FALSE;
}
static gboolean key_release_cb (GtkWidget *widget, GdkEventKey *event, 
    gpointer data) {
    g_print("key release: %d\n", event->keyval);
    // if (event->keyval == GDK_KEY_space){
    //     g_print("SPACE KEY PRESSED!\n");
    //     return TRUE;
    // }
    return TRUE;
}
static gboolean button_press_cb (GtkWidget *widget, GdkEventButton *event, 
    gpointer data) {
    g_print("click: %f, %f\n", event->x, event->y);
    return TRUE;
}
/* This creates all the GTK+ widgets that compose our application, and registers the callbacks */
static void create_ui (CustomData *data) {
  GtkWidget *main_window;  /* The uppermost window, containing all other windows */
  GtkWidget *video_window; /* The drawing area where the video will be shown */
  GtkWidget *main_box;     /* VBox to hold main_hbox and the controls */
  GtkWidget *main_hbox;    /* HBox to hold the video_window and the stream info text widget */

  main_window = gtk_window_new (GTK_WINDOW_TOPLEVEL);
  g_signal_connect (G_OBJECT (main_window), "delete-event", G_CALLBACK (delete_event_cb), data);

  video_window = gtk_drawing_area_new();
  //gtk_widget_set_double_buffered (video_window, FALSE);
  g_signal_connect (video_window, "realize", G_CALLBACK (realize_cb), data);
  g_signal_connect (video_window, "draw", G_CALLBACK (draw_cb), data);
  gtk_widget_add_events(video_window,  GDK_KEY_PRESS_MASK|GDK_KEY_RELEASE_MASK|GDK_BUTTON_PRESS_MASK);
  gtk_widget_set_can_focus(video_window, TRUE);
  g_signal_connect (video_window, "key-press-event", G_CALLBACK (key_press_cb), NULL);
  //g_signal_connect (video_window, "key-release-event", G_CALLBACK (key_release_cb), NULL);
  g_signal_connect (video_window, "button-press-event", G_CALLBACK (button_press_cb), NULL);

  main_hbox = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 0);
  gtk_box_pack_start (GTK_BOX (main_hbox), video_window, TRUE, TRUE, 0);
 
  main_box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  gtk_box_pack_start (GTK_BOX (main_box), main_hbox, TRUE, TRUE, 0);
  gtk_container_add (GTK_CONTAINER (main_window), main_box);
  gtk_window_set_default_size (GTK_WINDOW (main_window), 1200, 800);

  gtk_widget_show_all (main_window);
  //gdk_set_show_events (TRUE);

}

/* This function is called periodically to refresh the GUI */
static gboolean refresh_ui (CustomData *data) {
    g_print("refresing the gui...\n");
//   gint64 current = -1;

//   /* We do not want to update anything unless we are in the PAUSED or PLAYING states */
//   if (data->state < GST_STATE_PAUSED)
//     return TRUE;

//   /* If we didn't know it yet, query the stream duration */
//   if (!GST_CLOCK_TIME_IS_VALID (data->duration)) {
//     if (!gst_element_query_duration (data->pipeline, GST_FORMAT_TIME, &data->duration)) {
//       g_printerr ("Could not query current duration.\n");
//     } else {
//       /* Set the range of the slider to the clip duration, in SECONDS */
//       gtk_range_set_range (GTK_RANGE (data->slider), 0, (gdouble)data->duration / GST_SECOND);
//     }
//   }

//   if (gst_element_query_position (data->pipeline, GST_FORMAT_TIME, &current)) {
//     /* Block the "value-changed" signal, so the slider_cb function is not called
//      * (which would trigger a seek the user has not requested) */
//     g_signal_handler_block (data->slider, data->slider_update_signal_id);
//     /* Set the position of the slider to the current pipeline position, in SECONDS */
//     gtk_range_set_value (GTK_RANGE (data->slider), (gdouble)current / GST_SECOND);
//     /* Re-enable the signal */
//     g_signal_handler_unblock (data->slider, data->slider_update_signal_id);
//   }
//   return TRUE;
}

/* This function is called when an error message is posted on the bus */
static void error_cb (GstBus *bus, GstMessage *msg, CustomData *data) {
  GError *err;
  gchar *debug_info;

  /* Print error details on the screen */
  gst_message_parse_error (msg, &err, &debug_info);
  g_printerr ("Error received from element %s: %s\n", GST_OBJECT_NAME (msg->src), err->message);
  g_printerr ("Debugging information: %s\n", debug_info ? debug_info : "none");
  g_clear_error (&err);
  g_free (debug_info);

  /* Set the pipeline to READY (which stops playback) */
  gst_element_set_state (data->pipeline, GST_STATE_READY);
}

/* This function is called when an End-Of-Stream message is posted on the bus.
 * We just set the pipeline to READY (which stops playback) */
static void eos_cb (GstBus *bus, GstMessage *msg, CustomData *data) {
  g_print ("End-Of-Stream reached.\n");
  gst_element_set_state (data->pipeline, GST_STATE_READY);
}

/* This function is called when the pipeline changes states. We use it to
 * keep track of the current state. */
static void state_changed_cb (GstBus *bus, GstMessage *msg, CustomData *data) {
  GstState old_state, new_state, pending_state;
  gst_message_parse_state_changed (msg, &old_state, &new_state, &pending_state);
  if (GST_MESSAGE_SRC (msg) == GST_OBJECT (data->pipeline)) {
    data->state = new_state;
    g_print ("State set to %s\n", gst_element_state_get_name (new_state));
    if (old_state == GST_STATE_READY && new_state == GST_STATE_PAUSED) {
      /* For extra responsiveness, we refresh the GUI as soon as we reach the PAUSED state */
      refresh_ui (data);
    }
  }
}


int main(int argc, char *argv[]) {
    CustomData data;
    GstStateChangeReturn ret;
    GstBus *bus;

    /* Initialize GTK */
    gtk_init (&argc, &argv);

    /* Initialize GStreamer */
    gst_init (&argc, &argv);

    /* Initialize our data structure */
    memset (&data, 0, sizeof (data));
    data.duration = GST_CLOCK_TIME_NONE;

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


    /* Create the GUI */
    create_ui (&data);

    /* Instruct the bus to emit signals for each received message, and connect to the interesting signals */
    bus = gst_element_get_bus (data.pipeline);
    gst_bus_add_signal_watch (bus);
    g_signal_connect (G_OBJECT (bus), "message::error", (GCallback)error_cb, &data);
    g_signal_connect (G_OBJECT (bus), "message::eos", (GCallback)eos_cb, &data);
    g_signal_connect (G_OBJECT (bus), "message::state-changed", (GCallback)state_changed_cb, &data);
    gst_object_unref (bus);

    /* Start playing */
    ret = gst_element_set_state (data.pipeline, GST_STATE_PLAYING);
    if (ret == GST_STATE_CHANGE_FAILURE) {
        g_printerr ("Unable to set the pipeline to the playing state.\n");
        gst_object_unref (data.pipeline);
        return -1;
    }

    /* Register a function that GLib will call every second */
    g_timeout_add_seconds (1, (GSourceFunc)refresh_ui, &data);

    /* Start the GTK main loop. We will not regain control until gtk_main_quit is called. */
    gtk_main ();

    /* Free resources */
    gst_element_set_state (data.pipeline, GST_STATE_NULL);
    gst_object_unref (data.pipeline);
    return 0;
}