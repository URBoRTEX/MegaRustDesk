/* Tests the actual native implementation without opening the user's clipboard. */
#define CINTERFACE
#define COBJMACROS
#include <windows.h>
#include <ole2.h>
static int applied_count = 0;
static HRESULT fixture_ole_set(IDataObject *object) { ++applied_count; return S_OK; }
/* Substitute only the OS write; exercise the production window/ACK code. */
#define OleSetClipboard fixture_ole_set
#include "../../libs/clipboard/src/windows/wf_cliprdr.c"
#undef OleSetClipboard
#include <assert.h>

static volatile LONG64 test_token = 0;
static UINT64 access_token_for_test(CliprdrClientContext *context) { return (UINT64)InterlockedCompareExchange64(&test_token, 0, 0); }
static HANDLE wait_started;
static DWORD WINAPI interrupted_wait(void *argument)
{
    wfClipboard *fixture = (wfClipboard *)argument;
    BOOL received = FALSE;
    void *data = NULL;
    SetEvent(wait_started);
    return wait_response_event(1, fixture, fixture->req_fevent, &received, &data, 31);
}

static void pending_wait_revocation(void)
{
    CliprdrClientContext context = {0};
    wfClipboard fixture = {0};
    context.ClipboardAccessToken = access_token_for_test;
    context.ResponseWaitTimeoutSecs = 30;
    fixture.context = &context;
    fixture.req_fevent = CreateEvent(NULL, FALSE, FALSE, NULL);
    wait_started = CreateEvent(NULL, FALSE, FALSE, NULL);
    InterlockedExchange64(&test_token, 31);
    HANDLE thread = CreateThread(NULL, 0, interrupted_wait, &fixture, 0, NULL);
    assert(WaitForSingleObject(wait_started, 3000) == WAIT_OBJECT_0);
    Sleep(20);
    /* Even an immediate revoke/regrant must abort the original 30-second wait. */
    InterlockedExchange64(&test_token, 39);
    assert(WaitForSingleObject(thread, 3000) == WAIT_OBJECT_0);
    DWORD result = 0;
    assert(GetExitCodeThread(thread, &result) && result == ERROR_ACCESS_DENIED);
    CloseHandle(thread); CloseHandle(wait_started); CloseHandle(fixture.req_fevent);
}
static int ack_count = 0;
static BOOL no_clipboard_listener(HWND window) { return TRUE; }
static UINT applied_ack(CliprdrClientContext *context, const CLIPRDR_FORMAT_LIST_RESPONSE *response)
{
    assert(applied_count == 1);
    assert(response->dataLen == 91 && response->connID == 23);
    assert(response->msgFlags == CB_RESPONSE_OK);
    assert(cliprdr_callback_token() == 15);
    ++ack_count;
    return CHANNEL_RC_OK;
}

static void application_ack_order(void)
{
    CliprdrClientContext context = {0};
    wfClipboard fixture = {0};
    CREATESTRUCT create = {0};
    context.ClipboardAccessToken = access_token_for_test;
    context.ClientFormatListResponse = applied_ack;
    context.Custom = &fixture;
    fixture.context = &context;
    fixture.AddClipboardFormatListener = no_clipboard_listener;
    fixture.data_obj_mutex = CreateMutex(NULL, FALSE, NULL);
    context.EnableFiles = TRUE;
    fixture.map_capacity = 10;
    fixture.format_mappings = (formatMapping *)calloc(fixture.map_capacity, sizeof(formatMapping));
    create.lpCreateParams = &fixture;
    cliprdr_proc(NULL, WM_CREATE, 0, (LPARAM)&create);
    CLIPRDR_FORMAT formats[3] = {0};
    formats[0].formatId = 1001; formats[0].formatName = CFSTR_FILEDESCRIPTORW;
    formats[1].formatId = 1002; formats[1].formatName = CFSTR_FILECONTENTS;
    formats[2].formatId = 91; formats[2].formatName = "MegaRustDesk-Clipboard-Sequence";
    CLIPRDR_FORMAT_LIST list = {0};
    list.connID = 23; list.numFormats = 3; list.formats = formats;
    MSG message = {0};
    PeekMessage(&message, NULL, WM_CLIPRDR_MESSAGE, WM_CLIPRDR_MESSAGE, PM_NOREMOVE);
    assert(wf_cliprdr_server_format_list(&context, &list) == CHANNEL_RC_OK);
    assert(fixture.map_size == 2);
    assert(PeekMessage(&message, NULL, WM_CLIPRDR_MESSAGE, WM_CLIPRDR_MESSAGE, PM_REMOVE));
    assert(((UINT32 *)message.lParam)[1] == 91);
    assert(applied_count == 0 && ack_count == 0);
    cliprdr_proc(NULL, message.message, message.wParam, message.lParam);
    assert(applied_count == 1 && ack_count == 1);
    test_token = 23;
    UINT32 *payload = (UINT32 *)calloc(2, sizeof(UINT32));
    payload[0] = 23; payload[1] = 92;
    cliprdr_proc(NULL, WM_CLIPRDR_MESSAGE, (15ULL << 8) | OLE_SETCLIPBOARD, (LPARAM)payload);
    assert(applied_count == 1 && ack_count == 1);
    wf_destroy_file_obj(fixture.data_obj);
    clear_format_map(&fixture);
    free(fixture.format_mappings);
    CloseHandle(fixture.data_obj_mutex);
}

static const size_t file_sizes[3] = { 2 * 1024 * 1024 + 17, 0, 3073 };
static unsigned char file_byte(size_t file, size_t offset) { return (unsigned char)(offset * 17 + file * 43); }

static UINT synthetic_descriptors(CliprdrClientContext *context, const CLIPRDR_FORMAT_DATA_REQUEST *request)
{
    size_t size = sizeof(FILEGROUPDESCRIPTORW) + 2 * sizeof(FILEDESCRIPTORW);
    FILEGROUPDESCRIPTORW *group = (FILEGROUPDESCRIPTORW *)calloc(1, size);
    CLIPRDR_FORMAT_DATA_RESPONSE response = {0};
    group->cItems = 3;
    wcscpy_s(group->fgd[0].cFileName, MAX_PATH, L"\x0444\x0430\x0439\x043b.txt");
    wcscpy_s(group->fgd[1].cFileName, MAX_PATH, L"folder");
    wcscpy_s(group->fgd[2].cFileName, MAX_PATH, L"folder\\nested.bin");
    for (size_t i = 0; i < 3; ++i) {
        group->fgd[i].dwFlags = FD_ATTRIBUTES | FD_FILESIZE;
        group->fgd[i].dwFileAttributes = i == 1 ? FILE_ATTRIBUTE_DIRECTORY : FILE_ATTRIBUTE_NORMAL;
        group->fgd[i].nFileSizeLow = (DWORD)file_sizes[i];
    }
    response.connID = request->connID;
    response.msgFlags = CB_RESPONSE_OK;
    response.dataLen = (UINT32)size;
    response.requestedFormatData = (BYTE *)group;
    UINT result = wf_cliprdr_server_format_data_response(context, &response);
    free(group);
    return result;
}

static UINT synthetic_contents(CliprdrClientContext *context, const CLIPRDR_FILE_CONTENTS_REQUEST *request)
{
    CLIPRDR_FILE_CONTENTS_RESPONSE response = {0};
    size_t size = request->dwFlags == FILECONTENTS_SIZE ? 8 : request->cbRequested;
    BYTE *bytes = (BYTE *)calloc(1, size);
    size_t offset = ((UINT64)request->nPositionHigh << 32) | request->nPositionLow;
    assert(request->listIndex < 3);
    if (request->dwFlags == FILECONTENTS_SIZE) {
        UINT64 value = file_sizes[request->listIndex];
        memcpy(bytes, &value, 8);
    } else {
        if (offset + size > file_sizes[request->listIndex]) size = file_sizes[request->listIndex] - offset;
        for (size_t i = 0; i < size; ++i) bytes[i] = file_byte(request->listIndex, offset + i);
    }
    response.connID = request->connID;
    response.streamId = request->streamId;
    response.msgFlags = CB_RESPONSE_OK;
    response.cbRequested = (UINT32)size;
    response.requestedData = bytes;
    UINT result = wf_cliprdr_server_file_contents_response(context, &response);
    free(bytes);
    return result;
}

static void synthetic_native_paste(void)
{
    /* Only synthetic protocol responses: no OpenClipboard/OleGetClipboard call. */
    CliprdrClientContext context = {0};
    wfClipboard fixture = {0};
    formatMapping mapping = {0};
    IDataObject *object = NULL;
    UINT32 conn = 23;
    FORMATETC format = {0};
    STGMEDIUM medium = {0};
    context.ClipboardAccessToken = access_token_for_test;
    context.ClientFormatDataRequest = synthetic_descriptors;
    context.ClientFileContentsRequest = synthetic_contents;
    context.Custom = &fixture;
    context.ResponseWaitTimeoutSecs = 1;
    fixture.context = &context;
    fixture.copied = TRUE;
    fixture.formatDataRespEvent = CreateEvent(NULL, FALSE, FALSE, NULL);
    fixture.req_fevent = CreateEvent(NULL, FALSE, FALSE, NULL);
    mapping.local_format_id = RegisterClipboardFormat(CFSTR_FILEDESCRIPTORW);
    mapping.remote_format_id = 1001;
    fixture.format_mappings = &mapping;
    fixture.map_size = 1;
    assert(wf_create_file_obj(&conn, &fixture, &object));

    format.cfFormat = (CLIPFORMAT)mapping.local_format_id;
    format.dwAspect = DVASPECT_CONTENT;
    format.lindex = -1;
    format.tymed = TYMED_HGLOBAL;
    assert(IDataObject_GetData(object, &format, &medium) == S_OK);
    FILEGROUPDESCRIPTORW *group = (FILEGROUPDESCRIPTORW *)GlobalLock(medium.hGlobal);
    assert(group && group->cItems == 3);
    assert(wcscmp(group->fgd[0].cFileName, L"\x0444\x0430\x0439\x043b.txt") == 0);
    assert(group->fgd[1].dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY);
    GlobalUnlock(medium.hGlobal);
    HGLOBAL descriptor_memory = medium.hGlobal;

    for (size_t index = 0; index < 3; index += 2) {
        ZeroMemory(&medium, sizeof(medium));
        format.cfFormat = RegisterClipboardFormat(CFSTR_FILECONTENTS);
        format.tymed = TYMED_ISTREAM;
        format.lindex = (LONG)index;
        assert(IDataObject_GetData(object, &format, &medium) == S_OK);
        size_t position = 0;
        for (;;) {
            unsigned char bytes[4096];
            ULONG got = 0;
            HRESULT result = IStream_Read(medium.pstm, bytes, sizeof(bytes), &got);
            assert(result == S_OK || result == S_FALSE);
            for (ULONG i = 0; i < got; ++i) assert(bytes[i] == file_byte(index, position + i));
            position += got;
            if (!got) break;
        }
        assert(position == file_sizes[index]);
        IStream_Release(medium.pstm);
    }
    IDataObject_Release(object);
    GlobalFree(descriptor_memory);
    free(fixture.req_fdata);
    CloseHandle(fixture.formatDataRespEvent);
    CloseHandle(fixture.req_fevent);
}

int main(void)
{
    CliprdrClientContext context = {0};
    wfClipboard fixture = {0};
    FORMATETC format = {0};
    STGMEDIUM medium = {0};
    CliprdrDataObject object = {0};
    CliprdrStream stream = {0};
    ULONG count = 0;
    char byte = 0;
    context.ClipboardAccessToken = access_token_for_test;
    fixture.context = &context;
    context.Custom = &fixture;

    /* Denied operations must return before any HWND, HANDLE, path or payload use. */
    assert(cliprdr_send_format_list(&fixture, 1) == ERROR_ACCESS_DENIED);
    assert(cliprdr_send_data_request(1, &fixture, 1) == ERROR_ACCESS_DENIED);
    assert(cliprdr_send_request_filecontents(&fixture, 1, 0, 0, 0, 0, 0, 1) == ERROR_ACCESS_DENIED);
    assert(wf_cliprdr_server_format_list(&context, 0) == ERROR_ACCESS_DENIED);
    assert(wf_cliprdr_server_format_data_request(&context, 0) == ERROR_ACCESS_DENIED);
    assert(wf_cliprdr_server_file_contents_request(&context, 0) == ERROR_ACCESS_DENIED);
    assert(!wf_cliprdr_traverse_directory(&fixture, 0, 0));
    assert(!wf_cliprdr_add_to_file_arrays(&fixture, 0, 0));
    assert(wait_response_event(1, &fixture, 0, 0, 0, 7) == ERROR_ACCESS_DENIED);

    object.m_pData = &fixture;
    object.m_processID = GetCurrentProcessId();
    object.m_access_token = 7;
    stream.m_pData = &fixture;
    stream.m_access_token = 7;
    assert(CliprdrDataObject_GetData((IDataObject *)&object, &format, &medium) == E_ABORT);
    assert(CliprdrStream_Read((IStream *)&stream, &byte, 1, &count) == E_ABORT);

    /* Reclaim never revives old OLE objects or a pending wait. */
    test_token = 15;
    assert(CliprdrDataObject_GetData((IDataObject *)&object, &format, &medium) == E_ABORT);
    assert(CliprdrStream_Read((IStream *)&stream, &byte, 1, &count) == E_ABORT);
    assert(wait_response_event(1, &fixture, 0, 0, 0, 7) == ERROR_ACCESS_DENIED);
    assert(wf_cliprdr_same_access(&fixture, 15));
    synthetic_native_paste();
    synthetic_native_paste();
    application_ack_order();
    pending_wait_revocation();
    printf("PASS: native deny paths, stale OLE objects, stale waits; ABI size=%zu callbackOffset=%zu\n", sizeof(context), offsetof(CliprdrClientContext, ClipboardAccessToken));
    printf("PASS: native OLE synthetic paste x2, Unicode, directory, two files, 2100242 exact bytes/run\n");
    printf("PASS: apply ACK follows OLE write, stale queued application cannot ACK\n");
    printf("PASS: revoke/regrant aborts an active 30-second native wait\n");
    return 0;
}
