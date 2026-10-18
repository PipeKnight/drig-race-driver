package org.reassembly.grid;

import android.content.ContentResolver;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import android.provider.DocumentsContract.Document;

import java.io.FileNotFoundException;
import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.List;

/** A folder picked with ACTION_OPEN_DOCUMENT_TREE, read for {@link GridImporter}. */
final class SafTree implements GridImporter.Tree {
    private static final String[] COLUMNS = {
        Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME,
        Document.COLUMN_MIME_TYPE, Document.COLUMN_SIZE,
    };

    private final ContentResolver resolver;
    private final Uri tree;

    SafTree(ContentResolver resolver, Uri tree) {
        this.resolver = resolver;
        this.tree = tree;
    }

    @Override
    public GridImporter.Node root() {
        return new GridImporter.Node(DocumentsContract.getTreeDocumentId(tree), "", true, -1);
    }

    @Override
    public List<GridImporter.Node> children(GridImporter.Node directory) throws IOException {
        Uri uri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, directory.id);
        List<GridImporter.Node> nodes = new ArrayList<>();
        try (Cursor cursor = resolver.query(uri, COLUMNS, null, null, null)) {
            if (cursor == null) {
                throw new IOException("cannot list " + directory.name);
            }
            while (cursor.moveToNext()) {
                boolean folder = Document.MIME_TYPE_DIR.equals(cursor.getString(2));
                long size = cursor.isNull(3) ? -1 : cursor.getLong(3);
                nodes.add(new GridImporter.Node(cursor.getString(0), cursor.getString(1), folder, size));
            }
        } catch (SecurityException e) {
            throw new IOException("no access to the picked folder; pick it again", e);
        }
        return nodes;
    }

    @Override
    public InputStream open(GridImporter.Node file) throws IOException {
        Uri uri = DocumentsContract.buildDocumentUriUsingTree(tree, file.id);
        InputStream in;
        try {
            in = resolver.openInputStream(uri);
        } catch (SecurityException e) {
            throw new IOException("no access to " + file.name + "; pick the folder again", e);
        }
        if (in == null) {
            throw new FileNotFoundException(file.name);
        }
        return in;
    }
}
