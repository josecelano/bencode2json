/* converts Bencoded data on stdin to JSON on stdout */

#include <stdio.h>
#include <ctype.h>
#include <stdlib.h>

int pos = 0;
int c;
char stack[1024], *sp = stack;

void die()
{
    fflush(stdout);
    fprintf(stderr, "\nparse error at position %d: ", pos);
    if (c == EOF)
        fputs("got EOF\n", stderr);
    else
        fprintf(stderr, "found '%c'\n", (char)c);
    exit(1);
}

static inline int chomp()
{
    pos++;
    c = getchar();
    return c != EOF;
}

static inline void dump_int()
{
    int st = 0;

    while (chomp())
    {
        if (isdigit(c))
        {
            st = 2;
            putchar(c);
        }
        else if (c == 'e' && st == 2)
        {
            return;
        }
        else if (c == '-' && st == 0)
        {
            st = 1;
            putchar(c);
        }
        else
        {
            die();
        }
    }
}

static inline void dump_str(int len)
{
    int st = 0;

    len -= '0';
    putchar('"');
    while (chomp())
    {
        if (st == 0)
        {
            if (isdigit(c))
            {
                len = 10 * len + c - '0';
            }
            else if (c == ':')
            {
                st = 1;
                if (len == 0)
                    break;
            }
            else
                die();
        }
        else
        {
            len--;
            if (c == '"' || c == '\\')
            {
                putchar('\\');
            }
            putchar(c);
            if (len == 0)
                break;
        }
    }
    if (len > 0)
        die();
    putchar('"');
}

static void struct_hlp()
{
    switch (*sp)
    {
    case 'd':
        *sp = 'e';
        break;
    case 'e':
        putchar(':');
        *sp = 'f';
        break;
    case 'f':
        putchar(',');
        *sp = 'e';
        break;
    case 'l':
        *sp = 'm';
        break;
    case 'm':
        putchar(',');
        break;
    }
}

static inline void push(char ch)
{
    if (sp >= stack + sizeof(stack) - 1)
    {
        fputs("stack overflow\n", stderr);
        die();
    }
    *++sp = ch;
}

int main(int ac, char *av[])
{
    *sp = 'i';
    while (chomp())
    {
        switch ((char)c)
        {
        case 'd':
            struct_hlp();
            putchar('{');
            push((char)c);
            break;
        case 'l':
            struct_hlp();
            putchar('[');
            push((char)c);
            break;
        case 'i':
            struct_hlp();
            dump_int();
            break;
        case '0':
        case '1':
        case '2':
        case '3':
        case '4':
        case '5':
        case '6':
        case '7':
        case '8':
        case '9':
            struct_hlp();
            dump_str(c);
            break;
        case 'e':
            if (*sp == 'l' || *sp == 'm')
            {
                putchar(']');
                sp--;
            }
            else if (*sp == 'd' || *sp == 'f')
            {
                putchar('}');
                sp--;
            }
            else
                die();
            if (sp < stack)
                die();
            break;
        case '\n':
            break;
        default:
            die();
            break;
        }
    }
    if (sp != stack)
        die();
    putchar('\n');
    return 0;
}